package org.nmp.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Production cold-start identity-persistence regression guard.
 *
 * The Rust kernel restores the persisted identity automatically inside
 * `ActorCommand::Start` (`session_persistence::restore_active_session`) and
 * persists on sign-in (`enqueue_persist_current_active_session`). BOTH read and
 * write the secret exclusively through the keyring **capability** socket. If the
 * host never installs the `KeystoreKeyringCapability` before start, the persist
 * writes go nowhere and the restore on the next launch finds nothing — so a
 * signed-in user is silently logged out on every restart.
 *
 * This previously regressed because the keyring capability was only wired on the
 * `startWithContext` path, which production never reached. iOS installs the
 * keychain capability unconditionally before start; Android must mirror that.
 *
 * These tests drive the pure [planKernelLaunch] orchestration with a recording
 * [RecordingLaunchSeam] so they run on the JVM without the native `.so`.
 */
class KernelLaunchSequenceTest {

    /**
     * Records every launch-relevant operation in invocation order so the test
     * can assert the production path installs the keyring capability before
     * starting the kernel.
     */
    private class RecordingLaunchSeam : KernelLaunchSeam {
        val ops = mutableListOf<String>()
        var capabilityInstalled = false
        var seededRelays: String? = null
        var startStoragePath: String? = null

        override fun installKeyringCapability() {
            capabilityInstalled = true
            ops += "installKeyringCapability"
        }

        override fun startKernel(storagePath: String?, testRelays: String?) {
            startStoragePath = storagePath
            seededRelays = testRelays
            ops += "startKernel"
        }

        override fun wireListeners() {
            ops += "wireListeners"
        }
    }

    // ── Production path — the regression guard ────────────────────────────────

    @Test
    fun productionLaunchInstallsCapabilityBeforeKernelStart() {
        val seam = RecordingLaunchSeam()
        planKernelLaunch(
            seam = seam,
            storagePath = "/data/NMP",
            testRelays = null,
        )

        assertTrue(
            "production launch MUST install the keyring capability " +
                "(else persist + restore are silent no-ops)",
            seam.capabilityInstalled,
        )
        assertEquals(
            "keyring capability must be installed before the kernel Start command",
            listOf("installKeyringCapability", "startKernel", "wireListeners"),
            seam.ops,
        )
        // No test-relay override in production: the kernel seeds Chirp defaults.
        assertNull(seam.seededRelays)
    }

    @Test
    fun capabilityIsInstalledBeforeStart() {
        val seam = RecordingLaunchSeam()
        planKernelLaunch(
            seam = seam,
            storagePath = null,
            testRelays = null,
        )
        val installIdx = seam.ops.indexOf("installKeyringCapability")
        val startIdx = seam.ops.indexOf("startKernel")
        assertTrue("capability install must occur", installIdx >= 0)
        assertTrue("kernel start must occur", startIdx >= 0)
        assertTrue(
            "keyring capability must be installed BEFORE kernel start reads the persisted secret",
            installIdx < startIdx,
        )
        assertTrue(
            "listeners are wired after start",
            startIdx < seam.ops.indexOf("wireListeners"),
        )
    }

    // ── Test-injection path — layered on top, never a separate orchestration ──

    @Test
    fun relayInjectionLayersOnTopOfTheSameProductionPath() {
        val seam = RecordingLaunchSeam()
        planKernelLaunch(
            seam = seam,
            storagePath = "/data/NMP",
            testRelays = """[["ws://127.0.0.1:10547","both"]]""",
        )
        // Same unconditional capability + start wiring as production.
        assertTrue(seam.capabilityInstalled)
        assertEquals(listOf("installKeyringCapability", "startKernel", "wireListeners"), seam.ops)
        // The injected relay seam rides along.
        assertEquals("""[["ws://127.0.0.1:10547","both"]]""", seam.seededRelays)
    }
}
