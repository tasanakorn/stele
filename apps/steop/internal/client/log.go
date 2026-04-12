package client

// LogEvent is a structured log entry. ID is the 3-segment composite session id.
type LogEvent struct {
	ID    string      `json:"id"`
	Event string      `json:"event"`
	Data  interface{} `json:"data,omitempty"`
}

// Log appends a log entry. Uses fastClone for fire-and-forget semantics.
func (c *Client) Log(ev LogEvent) error {
	return c.fastClone().rpc("steop.log.append", ev, nil)
}
