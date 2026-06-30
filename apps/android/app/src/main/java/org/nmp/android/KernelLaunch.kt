package org.nmp.android

/**
 * The launch-relevant kernel operations, extracted behind an interface so the
 * cold-start orchestration ([planKernelLaunch]) is unit-testable on the JVM
 * without loading `libnmp_android_ffi.so`.
 *
 * The production implementation is [KernelModel]'s private adapter over its
 * [KernelBridge] + [KeystoreKeyringCapability]; tests substitute a recording
 * fake. The interface is deliberately ordered by lifecycle phase, mirroring iOS
 * `KernelModel` (`registerCapabilityHandler` before `start()`).
 */
interface KernelLaunchSeam {
    /**
     * Install the Android Keystore keyring capability so the kernel can both
     * persist the active secret on sign-in and read it back on cold start. Must
     * run BEFORE [startKernel] because the kernel actor's Start command runs
     * the synchronous restore-read chain.
     */
    fun installKeyringCapability()

    /** Start the kernel actor and seed relays ([testRelays] null = Chirp defaults). */
    fun startKernel(storagePath: String?, testRelays: String?)

    /** Wire the push update + signer-request listeners. */
    fun wireListeners()

}

/**
 * Cold-start launch orchestration — the SINGLE path used by both production and
 * UI tests.
 *
 * The keyring capability and identity-restore run **unconditionally**: this is
 * the fix for the v1-blocking regression where they were reachable only on the
 * `BuildConfig.DEBUG`-gated test path, so production installed no keyring (the
 * kernel's persist-on-sign-in writes and restore-on-start reads both route
 * through that capability) and logged the user out on every restart.
 *
 * [testRelays] is null in production and rides on top of the same path when a
 * headless UI test injects it — it never selects an alternate orchestration.
 *
 * Pure (no Android types): drives the [seam] in lifecycle order.
 */
fun planKernelLaunch(
    seam: KernelLaunchSeam,
    storagePath: String?,
    testRelays: String?,
) {
    // 1. Keyring capability first — the kernel's Start command (below) runs the
    //    synchronous restore-read chain, and restore reads through this socket.
    seam.installKeyringCapability()
    // 2. Start the kernel + seed relays (Start triggers the actor-side restore).
    seam.startKernel(storagePath, testRelays)
    // 3. Wire the push listeners so snapshots/signer requests flow.
    seam.wireListeners()
}

/**
 * Production [KernelLaunchSeam] adapter over the JNI [KernelBridge] plus the
 * freshly created [KeystoreKeyringCapability]. Thin — every method delegates
 * one-for-one to the bridge; the unconditional ordering lives in
 * [planKernelLaunch]. Extracted from `KernelModel` to keep that file under the
 * 500-LOC hard ceiling.
 *
 * [applyFrame] decodes + republishes a pushed update frame; [onSignerRequest]
 * routes a pushed NIP-55 request. Both are owned by `KernelModel` (they touch
 * its StateFlows / viewModelScope) and passed in so this adapter holds no UI
 * state of its own.
 */
class BridgeLaunchSeam(
    private val bridge: KernelBridge,
    private val capability: KeystoreKeyringCapability,
    private val applyFrame: (ByteArray) -> Unit,
    private val onSignerRequest: (String) -> Unit,
) : KernelLaunchSeam {
    override fun installKeyringCapability() {
        bridge.setCapabilityHandler(capability)
    }

    override fun startKernel(storagePath: String?, testRelays: String?) {
        if (!storagePath.isNullOrBlank()) {
            bridge.setStoragePath(storagePath)
        }
        bridge.start(visibleLimit = 80, emitHz = 4)
        bridge.seedRelays(testRelays)
    }

    override fun wireListeners() {
        bridge.setUpdateListener { bytes -> applyFrame(bytes) }
        bridge.setSignerRequestListener { requestJson -> onSignerRequest(requestJson) }
    }
}
