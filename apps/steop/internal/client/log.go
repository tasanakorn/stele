package client

// LogEvent is a structured log entry.
type LogEvent struct {
	Host       string      `json:"host"`
	ProjectDir string      `json:"project_dir"`
	SessionID  string      `json:"session_id"`
	Event      string      `json:"event"`
	Data       interface{} `json:"data,omitempty"`
}

// Log appends a log entry. Uses fastClone for fire-and-forget semantics.
func (c *Client) Log(ev LogEvent) error {
	return c.fastClone().rpc("steop.log.append", ev, nil)
}
