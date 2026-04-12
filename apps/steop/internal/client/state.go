package client

// State is an alias for Session — the session row holds all state.
type State = Session

// StateGet retrieves session state by full composite id.
func (c *Client) StateGet(id string) (*State, error) {
	body := map[string]string{"id": id}
	var out Session
	if err := c.rpc("steop.state.get", body, &out); err != nil {
		return nil, err
	}
	return &out, nil
}

// StatePut upserts the data JSON for a session (merge=true does shallow merge).
func (c *Client) StatePut(id string, data map[string]interface{}, merge bool) (*State, error) {
	body := map[string]interface{}{
		"id":    id,
		"data":  data,
		"merge": merge,
	}
	var out Session
	if err := c.rpc("steop.state.put", body, &out); err != nil {
		return nil, err
	}
	return &out, nil
}

// StateDelete deletes a session row.
func (c *Client) StateDelete(id string) (bool, error) {
	body := map[string]string{"id": id}
	var resp struct {
		Deleted bool `json:"deleted"`
	}
	if err := c.rpc("steop.state.delete", body, &resp); err != nil {
		return false, err
	}
	return resp.Deleted, nil
}

// CounterIncr atomically increments a named counter.
func (c *Client) CounterIncr(id, name string, delta int64) (int64, error) {
	body := map[string]interface{}{
		"id":      id,
		"counter": name,
		"delta":   delta,
	}
	var resp struct {
		Value int64 `json:"value"`
	}
	if err := c.rpc("steop.state.incr", body, &resp); err != nil {
		return 0, err
	}
	return resp.Value, nil
}

// CounterReset sets a named counter to a specific value.
func (c *Client) CounterReset(id, name string, value int64) (int64, error) {
	body := map[string]interface{}{
		"id":      id,
		"counter": name,
		"value":   value,
	}
	var resp struct {
		Value int64 `json:"value"`
	}
	if err := c.rpc("steop.state.reset", body, &resp); err != nil {
		return 0, err
	}
	return resp.Value, nil
}
