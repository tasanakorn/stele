package hooks

// HandlePermissionRequest is an observe-only handler in v1. It returns Allow()
// without contacting the server so the hook stays fast and non-blocking.
// Server-side observability will be wired up in a later version.
func HandlePermissionRequest(in *HookInput) []byte {
	return Allow()
}
