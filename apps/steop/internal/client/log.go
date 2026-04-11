package client

import "net/http"

// LogEvent is a structured event logged to the steop log endpoint.
type LogEvent struct {
	SessionID  string      `json:"session_id,omitempty"`
	Host       string      `json:"host,omitempty"`
	ProjectDir string      `json:"project_dir,omitempty"`
	Event      string      `json:"event"`
	Data       interface{} `json:"data,omitempty"`
}

// Log posts a structured log event using a short-timeout fast client.
func (c *Client) Log(ev LogEvent) error {
	if ev.Host == "" {
		ev.Host = c.host
	}
	if ev.ProjectDir == "" {
		ev.ProjectDir = c.projectDir
	}
	return c.fastClone().do(http.MethodPost, "/api/v1/steop/log", nil, ev, nil)
}

// InboxEnvelope is a cross-host session summary payload.
type InboxEnvelope struct {
	SessionID  string      `json:"session_id,omitempty"`
	Host       string      `json:"host,omitempty"`
	ProjectDir string      `json:"project_dir,omitempty"`
	Payload    interface{} `json:"payload,omitempty"`
}

// Inbox posts a session summary envelope using a short-timeout fast client.
func (c *Client) Inbox(env InboxEnvelope) error {
	if env.Host == "" {
		env.Host = c.host
	}
	if env.ProjectDir == "" {
		env.ProjectDir = c.projectDir
	}
	return c.fastClone().do(http.MethodPost, "/api/v1/steop/inbox", nil, env, nil)
}
