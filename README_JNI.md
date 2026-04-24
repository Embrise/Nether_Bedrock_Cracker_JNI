# Bedrock Cracker JNI

Based on [19MisterX98/Nether_Bedrock_Cracker](https://github.com/19MisterX98/Nether_Bedrock_Cracker), thanks to the original author.

基于 [19MisterX98/Nether_Bedrock_Cracker](https://github.com/19MisterX98/Nether_Bedrock_Cracker) 修改，感谢原作者。

---

[English](#english) | [中文](#中文)

---

## English

JNI integration branch of Nether_Bedrock_Cracker. Compiles to `bedrock_cracker.dll` for use by Fabric Mods (Kotlin) via JNI to crack Nether bedrock seeds.

### Build

Requires Rust toolchain and MSVC (Windows x86_64).

```bash
cargo build --release --features jni
```

Output: `target/release/bedrock_cracker.dll`

### Kotlin Integration

#### 1. Deploy DLL

The DLL must be on disk (cannot be loaded directly from JAR). Recommended: extract at runtime to a temp directory:

```kotlin
val tmpDir = Path.of(System.getProperty("java.io.tmpdir"), "bedrock_cracker")
tmpDir.toFile().mkdirs()
val dllFile = tmpDir.resolve("bedrock_cracker.dll")
if (!dllFile.toFile().exists()) {
    javaClass.getResourceAsStream("/natives/bedrock_cracker.dll")?.use { input ->
        dllFile.toFile().outputStream().use { output -> input.copyTo(output) }
    }
}
System.load(dllFile.toAbsolutePath().toString())
```

#### 2. Callback Interface

```kotlin
package xyz.embrise.client.jni

interface BedrockCrackerCallback {
    /** Called when a matching seed is found. Return true to continue, false to cancel. */
    fun onSeed(seed: Long): Boolean

    /** Called periodically to report progress. Return true to continue, false to cancel. */
    fun onProgress(seedsScanned: Long): Boolean
}
```

#### 3. API

```kotlin
package xyz.embrise.client.jni

object BedrockCracker {

    data class Block(val x: Int, val y: Int, val z: Int, val isBedrock: Boolean)

    enum class BedrockGeneration(val code: Int) {
        NORMAL(0),      // Standard Java Edition
        PAPER_1_18(1),  // PaperMC < 1.19.2-213
    }

    enum class OutputMode(val code: Int) {
        WORLD_SEED(0),     // Output world seed
        STRUCTURE_SEED(1), // Output structure seed
    }

    /** Estimate matching seed count. Synchronous, returns in microseconds. */
    fun estimateResultAmount(blocks: List<Block>): Long {
        return estimateResultAmountNative(flattenBlocks(blocks))
    }

    /** Full search over 2^48 seed space. Blocking — must run on a background thread. */
    fun searchBedrockPattern(
        blocks: List<Block>,
        threadCount: Int,
        mode: BedrockGeneration,
        outputMode: OutputMode,
        callback: BedrockCrackerCallback,
    ) {
        nativeSearchBedrockPattern(flattenBlocks(blocks), threadCount, mode.code, outputMode.code, callback)
    }

    /** Filter a given list of seeds. Blocking. */
    fun searchBedrockPatternWithList(
        blocks: List<Block>,
        threadCount: Int,
        seedList: LongArray,
        mode: BedrockGeneration,
        callback: BedrockCrackerCallback,
    ) {
        nativeSearchBedrockPatternWithList(flattenBlocks(blocks), threadCount, seedList, mode.code, callback)
    }

    private fun flattenBlocks(blocks: List<Block>): LongArray {
        val flat = LongArray(blocks.size * 4)
        blocks.forEachIndexed { i, b ->
            flat[i * 4 + 0] = b.x.toLong()
            flat[i * 4 + 1] = b.y.toLong()
            flat[i * 4 + 2] = b.z.toLong()
            flat[i * 4 + 3] = if (b.isBedrock) 0L else 1L
        }
        return flat
    }

    private external fun estimateResultAmountNative(blocks: LongArray): Long
    private external fun nativeSearchBedrockPattern(blocks: LongArray, threadCount: Int, mode: Int, outputMode: Int, callback: BedrockCrackerCallback)
    private external fun nativeSearchBedrockPatternWithList(blocks: LongArray, threadCount: Int, seedList: LongArray, mode: Int, callback: BedrockCrackerCallback)
}
```

#### 4. Usage Example

```kotlin
// Load DLL (call once during mod initialization)
System.load(dllPath.toAbsolutePath().toString())

val blocks = listOf(
    BedrockCracker.Block(x = -1, y = 123, z = -7, isBedrock = true),
    BedrockCracker.Block(x = 14, y = 4, z = -97, isBedrock = false),
)

// Estimate matching count
val estimate = BedrockCracker.estimateResultAmount(blocks)

// Search (run in coroutine or background thread)
launch(Dispatchers.IO) {
    BedrockCracker.searchBedrockPattern(
        blocks = blocks,
        threadCount = Runtime.getRuntime().availableProcessors(),
        mode = BedrockCracker.BedrockGeneration.NORMAL,
        outputMode = BedrockCracker.OutputMode.WORLD_SEED,
        callback = object : BedrockCrackerCallback {
            override fun onSeed(seed: Long): Boolean {
                println("Found seed: $seed")
                return true
            }
            override fun onProgress(seedsScanned: Long): Boolean = true
        },
    )
}
```

### Block Collection Tips

- Collect bedrock at **y=4** (floor) and **y=123** (ceiling) for maximum information
- Blocks must be from **different (x, z) coordinates** — only one per column is useful
- **20–25** high-quality blocks are enough to narrow down to a unique seed
- Avoid y=0 and y=127 (100% bedrock, zero information)

### Thread Safety

- `onSeed`/`onProgress` are called from **Rust worker threads**, not the Minecraft main thread
- To update UI, schedule on the main thread: `MinecraftClient.getInstance().execute { ... }`
- Multiple Rust threads may call back simultaneously — callbacks must be thread-safe
- Search functions block the calling thread — must be called from a coroutine or background thread

---

## 中文

Nether_Bedrock_Cracker 的 JNI 集成分支。编译为 `bedrock_cracker.dll`，供 Fabric Mod（Kotlin）通过 JNI 调用进行下界基岩种子破解。

### 编译

需要 Rust 工具链和 MSVC（Windows x86_64）。

```bash
cargo build --release --features jni
```

产物：`target/release/bedrock_cracker.dll`

### Kotlin 集成

#### 1. 部署 DLL

DLL 必须在磁盘上（不能从 JAR 内直接加载）。推荐运行时解压到临时目录：

```kotlin
val tmpDir = Path.of(System.getProperty("java.io.tmpdir"), "bedrock_cracker")
tmpDir.toFile().mkdirs()
val dllFile = tmpDir.resolve("bedrock_cracker.dll")
if (!dllFile.toFile().exists()) {
    javaClass.getResourceAsStream("/natives/bedrock_cracker.dll")?.use { input ->
        dllFile.toFile().outputStream().use { output -> input.copyTo(output) }
    }
}
System.load(dllFile.toAbsolutePath().toString())
```

#### 2. 回调接口

```kotlin
package xyz.embrise.client.jni

interface BedrockCrackerCallback {
    /** 找到匹配种子时调用。返回 true 继续，false 取消搜索。 */
    fun onSeed(seed: Long): Boolean

    /** 定期报告进度。返回 true 继续，false 取消搜索。 */
    fun onProgress(seedsScanned: Long): Boolean
}
```

#### 3. 调用 API

```kotlin
package xyz.embrise.client.jni

object BedrockCracker {

    data class Block(val x: Int, val y: Int, val z: Int, val isBedrock: Boolean)

    enum class BedrockGeneration(val code: Int) {
        NORMAL(0),      // 标准 Java 版
        PAPER_1_18(1),  // PaperMC < 1.19.2-213
    }

    enum class OutputMode(val code: Int) {
        WORLD_SEED(0),     // 输出世界种子
        STRUCTURE_SEED(1), // 输出结构种子
    }

    /** 预估匹配种子数。同步调用，微秒级返回。 */
    fun estimateResultAmount(blocks: List<Block>): Long {
        return estimateResultAmountNative(flattenBlocks(blocks))
    }

    /** 全量搜索 2^48 种子空间。阻塞调用，必须在后台线程执行。 */
    fun searchBedrockPattern(
        blocks: List<Block>,
        threadCount: Int,
        mode: BedrockGeneration,
        outputMode: OutputMode,
        callback: BedrockCrackerCallback,
    ) {
        nativeSearchBedrockPattern(flattenBlocks(blocks), threadCount, mode.code, outputMode.code, callback)
    }

    /** 列表过滤：只检查给定种子是否匹配。阻塞调用。 */
    fun searchBedrockPatternWithList(
        blocks: List<Block>,
        threadCount: Int,
        seedList: LongArray,
        mode: BedrockGeneration,
        callback: BedrockCrackerCallback,
    ) {
        nativeSearchBedrockPatternWithList(flattenBlocks(blocks), threadCount, seedList, mode.code, callback)
    }

    private fun flattenBlocks(blocks: List<Block>): LongArray {
        val flat = LongArray(blocks.size * 4)
        blocks.forEachIndexed { i, b ->
            flat[i * 4 + 0] = b.x.toLong()
            flat[i * 4 + 1] = b.y.toLong()
            flat[i * 4 + 2] = b.z.toLong()
            flat[i * 4 + 3] = if (b.isBedrock) 0L else 1L
        }
        return flat
    }

    private external fun estimateResultAmountNative(blocks: LongArray): Long
    private external fun nativeSearchBedrockPattern(blocks: LongArray, threadCount: Int, mode: Int, outputMode: Int, callback: BedrockCrackerCallback)
    private external fun nativeSearchBedrockPatternWithList(blocks: LongArray, threadCount: Int, seedList: LongArray, mode: Int, callback: BedrockCrackerCallback)
}
```

#### 4. 使用示例

```kotlin
// 加载 DLL（Mod 初始化时调用一次）
System.load(dllPath.toAbsolutePath().toString())

val blocks = listOf(
    BedrockCracker.Block(x = -1, y = 123, z = -7, isBedrock = true),
    BedrockCracker.Block(x = 14, y = 4, z = -97, isBedrock = false),
)

// 预估匹配数
val estimate = BedrockCracker.estimateResultAmount(blocks)

// 搜索（在协程或后台线程中调用）
launch(Dispatchers.IO) {
    BedrockCracker.searchBedrockPattern(
        blocks = blocks,
        threadCount = Runtime.getRuntime().availableProcessors(),
        mode = BedrockCracker.BedrockGeneration.NORMAL,
        outputMode = BedrockCracker.OutputMode.WORLD_SEED,
        callback = object : BedrockCrackerCallback {
            override fun onSeed(seed: Long): Boolean {
                println("找到种子: $seed")
                return true
            }
            override fun onProgress(seedsScanned: Long): Boolean = true
        },
    )
}
```

### 方块采集建议

- 采集 **y=4**（地板）和 **y=123**（天花板）的基岩，信息量最大
- 方块必须来自**不同的 (x, z) 坐标**，同一柱子只提供一个有效数据
- **20~25 个**高质量方块即可锁定唯一种子
- 避免 y=0 和 y=127（100% 基岩，零信息量）

### 线程安全

- `onSeed`/`onProgress` 在 **Rust 工作线程**中调用，不是 Minecraft 主线程
- 更新 UI 需调度到主线程：`MinecraftClient.getInstance().execute { ... }`
- 多个 Rust 线程会同时回调，回调实现必须是线程安全的
- 搜索函数会阻塞调用线程，必须在协程或后台线程中调用
