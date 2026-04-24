# Bedrock Cracker JNI

Nether_Bedrock_Cracker 的 JNI 集成分支。编译为 `bedrock_cracker.dll`，供 Fabric Mod（Kotlin）通过 JNI 调用进行下界基岩种子破解。

## 编译

需要 Rust 工具链和 MSVC（Windows x86_64）。

```bash
cargo build --release --features jni
```

产物：`target/release/bedrock_cracker.dll`

## Kotlin 集成

### 1. 部署 DLL

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

### 2. 回调接口

```kotlin
package xyz.embrise.client.jni

interface BedrockCrackerCallback {
    /** 找到匹配种子时调用。返回 true 继续，false 取消搜索。 */
    fun onSeed(seed: Long): Boolean

    /** 定期报告进度。返回 true 继续，false 取消搜索。 */
    fun onProgress(seedsScanned: Long): Boolean
}
```

### 3. 调用 API

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

### 4. 使用示例

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

## 方块采集建议

- 采集 **y=4**（地板）和 **y=123**（天花板）的基岩，信息量最大
- 方块必须来自**不同的 (x, z) 坐标**，同一柱子只提供一个有效数据
- **20~25 个**高质量方块即可锁定唯一种子
- 避免 y=0 和 y=127（100% 基岩，零信息量）

## 线程安全

- `onSeed`/`onProgress` 在 **Rust 工作线程**中调用，不是 Minecraft 主线程
- 更新 UI 需调度到主线程：`MinecraftClient.getInstance().execute { ... }`
- 多个 Rust 线程会同时回调，回调实现必须是线程安全的
- 搜索函数会阻塞调用线程，必须在协程或后台线程中调用

## 许可证

本项目基于 LGPL 协议。闭源作品可通过动态链接使用编译产物，但对本库源码的修改必须公开。
