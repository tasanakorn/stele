package client

import "net/http"

// Status is the statusline read projection returned by /api/v1/steop/status/:id.
// The endpoint never 404s — when no row exists the server returns a defaulted
// Status so callers can render unconditionally.
type Status struct {
	SessionID string `json:"session_id"`
	Mode      string `json:"mode"`
	Phase     string `json:"phase"`
	Step      string `json:"step"`
	ToolCalls int64  `json:"tool_calls"`
	LoopCount int64  `json:"loop_count"`
	StepRetry int64  `json:"step_retry"`
	UpdatedAt string `json:"updated_at"`
}

func (c *Client) StatusGet(sessionID string) (*Status, error) {
	var out Status
	if err := c.do(http.MethodGet, "/api/v1/steop/status/"+sessionID, nil, nil, &out); err != nil {
		return nil, err
	}
	return &out, nil
}
