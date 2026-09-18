import java.io.File
import org.apache.tools.ant.taskdefs.condition.Os
import org.gradle.api.DefaultTask
import org.gradle.api.GradleException
import org.gradle.api.logging.LogLevel
import org.gradle.api.tasks.Input
import org.gradle.api.tasks.TaskAction

open class BuildTask : DefaultTask() {
    @Input
    var rootDirRel: String? = null
    @Input
    var target: String? = null
    @Input
    var release: Boolean? = null

    @TaskAction
    fun assemble() {
        val target = target ?: throw GradleException("target cannot be null")
        val archMap = mapOf(
            "aarch64" to "arm64-v8a",
            "armv7" to "armeabi-v7a",
            "i686" to "x86",
            "x86_64" to "x86_64"
        )
        val abi = archMap[target] ?: target
        val jniLibDir = File(project.projectDir, "src/main/jniLibs/$abi")
        val soFiles = jniLibDir.listFiles { _, name -> name.endsWith(".so") }
        if (soFiles != null && soFiles.isNotEmpty()) {
            project.logger.lifecycle("Pre-built Rust library found for $target in $jniLibDir, skipping rustBuild.")
            return
        }

        // Determine working directory with package.json
        val rel = rootDirRel ?: throw GradleException("rootDirRel cannot be null")
        var workingDir = File(project.projectDir, rel)
        if (!File(workingDir, "package.json").exists()) {
            val parent = workingDir.parentFile
            if (parent != null && File(parent, "package.json").exists()) {
                workingDir = parent
            }
        }

        val release = release ?: throw GradleException("release cannot be null")
        val tauriArgs = mutableListOf("run", "--", "tauri", "android", "android-studio-script")
        if (project.logger.isEnabled(LogLevel.DEBUG)) {
            tauriArgs.add("-vv")
        } else if (project.logger.isEnabled(LogLevel.INFO)) {
            tauriArgs.add("-v")
        }
        if (release) {
            tauriArgs.add("--release")
        }
        tauriArgs.addAll(listOf("--target", target))

        if (Os.isFamily(Os.FAMILY_WINDOWS)) {
            val cmdArgs = mutableListOf("/c", "npm")
            cmdArgs.addAll(tauriArgs)
            project.exec {
                workingDir(workingDir)
                executable("cmd.exe")
                args(cmdArgs)
            }.assertNormalExitValue()
        } else {
            project.exec {
                workingDir(workingDir)
                executable("npm")
                args(tauriArgs)
            }.assertNormalExitValue()
        }
    }
}