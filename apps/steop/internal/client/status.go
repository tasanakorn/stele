package client

// Status holds the statusline projection for a session.
type Status struct {
	ID           string `json:"id"`
	Mode         string `json:"mode"`
	Phase        string `json:"phase"`
	Step         string `json:"step"`
	ToolCalls    int64  `json:"tool_calls"`
	LoopCount    int64  `json:"loop_count"`
	StepRetry    int64  `json:"step_retry"`
	LastActiveAt string `json:"last_active_at"`
}

// StatusGet retrieves the statusline projection (never returns 404 — missing sessions return defaults).
func (c *Client) StatusGet(id string) (*Status, error) {
	body := map[string]string{"id": id}
	var out Status
	if err := c.rpc("steop.status.get", body, &out); err != nil {
		return nil, err
	}
	return &out, nil
}
