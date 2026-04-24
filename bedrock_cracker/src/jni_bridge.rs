use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use jni::objects::{GlobalRef, JClass, JLongArray, JObject, JValue};
use jni::sys::{jint, jlong};
use jni::{JNIEnv, JavaVM};

use crate::raw_data::block::Block;
use crate::raw_data::block_type::BlockType;
use crate::raw_data::modes::{BedrockGeneration, OutputMode};
use crate::raw_data::sender::Sender;
use crate::{estimate_result_amount, search_bedrock_pattern, search_bedrock_pattern_with_list, CrackProgress};

// ---------------------------------------------------------------------------
// JniSender: implements Sender, calls back into Kotlin
// ---------------------------------------------------------------------------

struct JniSender {
    jvm: Arc<JavaVM>,
    callback: Arc<GlobalRef>,
    remaining: Arc<AtomicUsize>,
}

impl Clone for JniSender {
    fn clone(&self) -> Self {
        self.remaining.fetch_add(1, Ordering::Relaxed);
        Self {
            jvm: Arc::clone(&self.jvm),
            callback: Arc::clone(&self.callback),
            remaining: Arc::clone(&self.remaining),
        }
    }
}

impl Drop for JniSender {
    fn drop(&mut self) {
        self.remaining.fetch_sub(1, Ordering::Release);
    }
}

unsafe impl Send for JniSender {}
unsafe impl Sync for JniSender {}

impl Sender for JniSender {
    fn send(&self, progress: CrackProgress) -> bool {
        let Ok(mut env) = self.jvm.attach_current_thread_permanently() else {
            return false;
        };

        let (method, value) = match progress {
            CrackProgress::Seed(seed) => ("onSeed", seed as i64),
            CrackProgress::Progress(count) => ("onProgress", count as i64),
        };

        match env.call_method(&*self.callback, method, "(J)Z", &[JValue::Long(value)]) {
            Ok(ret) => ret.z().unwrap_or(false),
            Err(_) => {
                let _ = self.jvm.attach_current_thread_permanently().map(|e| e.exception_clear());
                false
            }
        }
    }
}

impl JniSender {
    fn new(env: &mut JNIEnv, callback: JObject, _thread_count: usize) -> Result<Self, jni::errors::Error> {
        let jvm = Arc::new(env.get_java_vm()?);
        let global_ref = env.new_global_ref(&callback)?;
        Ok(Self {
            jvm,
            callback: Arc::new(global_ref),
            remaining: Arc::new(AtomicUsize::new(1)),
        })
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse_blocks(env: &mut JNIEnv, array: &JLongArray) -> Option<Vec<Block>> {
    let len = env.get_array_length(array).ok()? as usize;
    if len == 0 || len % 4 != 0 {
        return None;
    }
    let mut buf = vec![0i64; len];
    env.get_long_array_region(array, 0, &mut buf).ok()?;
    Some(
        buf.chunks_exact(4)
            .map(|c| {
                Block::new(
                    c[0] as i32,
                    c[1] as i32,
                    c[2] as i32,
                    if c[3] == 0 { BlockType::BEDROCK } else { BlockType::OTHER },
                )
            })
            .collect(),
    )
}

fn parse_seed_list(env: &mut JNIEnv, array: &JLongArray) -> Option<Vec<u64>> {
    let len = env.get_array_length(array).ok()? as usize;
    if len == 0 {
        return None;
    }
    let mut buf = vec![0i64; len];
    env.get_long_array_region(array, 0, &mut buf).ok()?;
    Some(buf.iter().map(|&v| v as u64).collect())
}

fn parse_mode(mode: jint) -> Option<BedrockGeneration> {
    match mode {
        0 => Some(BedrockGeneration::Normal),
        1 => Some(BedrockGeneration::Paper1_18),
        _ => None,
    }
}

fn parse_output_mode(mode: jint) -> Option<OutputMode> {
    match mode {
        0 => Some(OutputMode::WorldSeed),
        1 => Some(OutputMode::StructureSeed),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// JNI Exports
// ---------------------------------------------------------------------------

#[no_mangle]
pub extern "system" fn Java_xyz_embrise_client_jni_BedrockCracker_estimateResultAmountNative(
    mut env: JNIEnv,
    _class: JClass,
    blocks_array: JLongArray,
) -> jlong {
    match parse_blocks(&mut env, &blocks_array) {
        Some(blocks) => estimate_result_amount(&blocks) as jlong,
        None => {
            let _ = env.exception_clear();
            0
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_xyz_embrise_client_jni_BedrockCracker_nativeSearchBedrockPattern(
    mut env: JNIEnv,
    _class: JClass,
    blocks_array: JLongArray,
    thread_count: jint,
    mode: jint,
    output_mode: jint,
    callback: JObject,
) {
    let tc = if thread_count > 0 { thread_count as u64 } else { return };

    let Some(blocks) = parse_blocks(&mut env, &blocks_array) else { return };
    let Some(mode) = parse_mode(mode) else { return };
    let Some(output) = parse_output_mode(output_mode) else { return };
    let Ok(sender) = JniSender::new(&mut env, callback, tc as usize) else { return };

    let remaining = Arc::clone(&sender.remaining);
    search_bedrock_pattern(&blocks, tc, mode, output, sender);

    // Block until all worker threads have finished (remaining drops to 0).
    while remaining.load(Ordering::Acquire) > 0 {
        std::thread::yield_now();
    }
}

#[no_mangle]
pub extern "system" fn Java_xyz_embrise_client_jni_BedrockCracker_nativeSearchBedrockPatternWithList(
    mut env: JNIEnv,
    _class: JClass,
    blocks_array: JLongArray,
    thread_count: jint,
    seed_list_array: JLongArray,
    mode: jint,
    callback: JObject,
) {
    let tc = if thread_count > 0 { thread_count as u64 } else { return };

    let Some(blocks) = parse_blocks(&mut env, &blocks_array) else { return };
    let Some(seeds) = parse_seed_list(&mut env, &seed_list_array) else { return };
    let Some(mode) = parse_mode(mode) else { return };
    let Ok(sender) = JniSender::new(&mut env, callback, tc as usize) else { return };

    let remaining = Arc::clone(&sender.remaining);
    search_bedrock_pattern_with_list(&blocks, tc, &seeds, mode, sender);

    while remaining.load(Ordering::Acquire) > 0 {
        std::thread::yield_now();
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_blocks_layout() {
        // Simulate what JNI would give us: [x0,y0,z0,type0, x1,y1,z1,type1]
        // We test the parsing logic directly by checking the Block construction.
        let flat: [i64; 8] = [-1, 123, -7, 0, 14, 4, 97, 1];
        let blocks: Vec<Block> = flat
            .chunks_exact(4)
            .map(|c| {
                Block::new(
                    c[0] as i32,
                    c[1] as i32,
                    c[2] as i32,
                    if c[3] == 0 { BlockType::BEDROCK } else { BlockType::OTHER },
                )
            })
            .collect();

        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].x, -1);
        assert_eq!(blocks[0].y, 123);
        assert_eq!(blocks[0].z, -7);
        assert_eq!(blocks[0].block_type, BlockType::BEDROCK);
        assert_eq!(blocks[1].x, 14);
        assert_eq!(blocks[1].y, 4);
        assert_eq!(blocks[1].z, 97);
        assert_eq!(blocks[1].block_type, BlockType::OTHER);
    }

    #[test]
    fn test_parse_seed_list_layout() {
        let flat: [i64; 3] = [765906787396911863, -8846219896077660381, 42];
        let seeds: Vec<u64> = flat.iter().map(|&v| v as u64).collect();

        assert_eq!(seeds.len(), 3);
        assert_eq!(seeds[0], 765906787396911863u64);
        // Interpretation of negative i64 as u64
        assert_eq!(seeds[1], (-8846219896077660381i64) as u64);
    }

    #[test]
    fn test_mode_parsing() {
        assert!(matches!(parse_mode(0), Some(BedrockGeneration::Normal)));
        assert!(matches!(parse_mode(1), Some(BedrockGeneration::Paper1_18)));
        assert!(parse_mode(2).is_none());
    }

    #[test]
    fn test_output_mode_parsing() {
        assert!(matches!(parse_output_mode(0), Some(OutputMode::WorldSeed)));
        assert!(matches!(parse_output_mode(1), Some(OutputMode::StructureSeed)));
        assert!(parse_output_mode(2).is_none());
    }
}
