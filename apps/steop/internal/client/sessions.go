package client

// Session represents a steop v0.7 session row. The composite id is the
// 3-segment "host:project_dir:session_id" string.
type Session struct {
	ID           string                 `json:"id"`
	State        string                 `json:"state"`
	StartedAt    string                 `json:"started_at"`
	LastActiveAt string                 `json:"last_active_at"`
	StoppedAt    *string                `json:"stopped_at"`
	Data         map[string]interface{} `json:"data"`
	Counters     map[string]int64       `json:"counters"`
}

// Project represents a 2-segment composite project id.
type Project struct {
	ID string `json:"id"`
}

// SessionStart creates or reactivates a session.
func (c *Client) SessionStart(id string, data map[string]interface{}) (*Session, error) {
	body := map[string]interface{}{"id": id}
	if data != nil {
		body["data"] = data
	}
	var out Session
	if err := c.rpc("steop.session.start", body, &out); err != nil {
		return nil, err
	}
	return &out, nil
}

// SessionStop marks a session as stopped.
func (c *Client) SessionStop(id string) (*Session, error) {
	body := map[string]string{"id": id}
	var out Session
	if err := c.rpc("steop.session.stop", body, &out); err != nil {
		return nil, err
	}
	return &out, nil
}

// SessionTouch refreshes last_active_at on a session.
func (c *Client) SessionTouch(id string) (*Session, error) {
	body := map[string]string{"id": id}
	var out Session
	if err := c.rpc("steop.session.touch", body, &out); err != nil {
		return nil, err
	}
	return &out, nil
}

// SessionGet retrieves a session by full composite id (3-segment).
func (c *Client) SessionGet(id string) (*Session, error) {
	body := map[string]string{"id": id}
	var out Session
	if err := c.rpc("steop.session.get", body, &out); err != nil {
		return nil, err
	}
	return &out, nil
}

// SessionList lists sessions, filtering by host/projectDir/state. Empty strings are omitted.
func (c *Client) SessionList(host, projectDir, state string, limit int) ([]Session, error) {
	body := map[string]interface{}{}
	if host != "" {
		body["host"] = host
	}
	if projectDir != "" {
		body["project_dir"] = projectDir
	}
	if state != "" {
		body["state"] = state
	}
	if limit > 0 {
		body["limit"] = limit
	}
	var resp struct {
		Sessions []Session `json:"sessions"`
	}
	if err := c.rpc("steop.session.list", body, &resp); err != nil {
		return nil, err
	}
	return resp.Sessions, nil
}

// ProjectList lists distinct 2-segment project ids.
func (c *Client) ProjectList(host string) ([]Project, error) {
	body := map[string]interface{}{}
	if host != "" {
		body["host"] = host
	}
	var resp struct {
		Projects []Project `json:"projects"`
	}
	if err := c.rpc("steop.project.list", body, &resp); err != nil {
		return nil, err
	}
	return resp.Projects, nil
}
