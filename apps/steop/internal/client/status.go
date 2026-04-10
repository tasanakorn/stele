package client

import "net/http"

type HudStatus struct {
	SessionID string `json:"session_id"`
	Mode      string `json:"mode"`
	Phase     string `json:"phase"`
	Step      string `json:"step"`
	ToolCalls int64  `json:"tool_calls"`
	LoopCount int64  `json:"loop_count"`
	StepRetry int64  `json:"step_retry"`
	UpdatedAt string `json:"updated_at"`
}

func (c *Client) StatusGet(sessionID string) (*HudStatus, error) {
	var out HudStatus
	if err := c.do(http.MethodGet, "/api/v1/steop/status/"+sessionID, nil, nil, &out); err != nil {
		return nil, err
	}
	return &out, nil
}
