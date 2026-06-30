import Foundation
import os.log

private let pgLog = Logger(subsystem: "io.f7z.chirp", category: "PublicGroupBridge")

extension KernelHandle {
    /// Dispatch a `nmp.nip29.create_public_group` action via the typed byte
    /// doorway (`nmp_app_dispatch_action_bytes`, #2170). Defaults to
    /// `GroupVisibility::Public (0)` and `GroupAccess::Open (0)` — the
    /// current Chirp UI does not expose those fields. Returns the decoded
    /// `DispatchResult`.
    @discardableResult
    func createPublicGroup(group: GroupId, name: String, about: String?) -> DispatchResult {
        let id = UUID().uuidString
        let bytes = GeneratedActionBuilders.createPublicGroup(
            correlationId: id,
            group: (hostRelayUrl: group.hostRelayUrl, localId: group.localId),
            name: name,
            about: about.flatMap { $0.isEmpty ? nil : $0 },
            picture: nil,
            visibility: 0, // GroupVisibility::Public
            access: 0,     // GroupAccess::Open
            parent: nil
        )
        return dispatchBytes(bytes)
    }
}
