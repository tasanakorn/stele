package client

import (
	"net/http"
)

type State struct {
	SessionID string                 `json:"session_id"`
	Data      map[string]interface{} `json:"data"`
	Counters  map[string]int64       `json:"counters"`
	CreatedAt string                 `json:"created_at"`
	UpdatedAt string                 `json:"updated_at"`
}

type statePutBody struct {
	Data  map[string]interface{} `json:"data"`
	Merge bool                   `json:"merge"`
}

type counterIncrBody struct {
	Counter string `json:"counter"`
	Delta   int64  `json:"delta"`
}

type counterResetBody struct {
	Counter string `json:"counter"`
	Value   int64  `json:"value"`
}

type counterResp struct {
	Counter string `json:"counter"`
	Value   int64  `json:"value"`
}

type stateDeleteResp struct {
	Deleted bool `json:"deleted"`
}

func (c *Client) StateGet(sessionID string) (*State, error) {
	var out State
	if err := c.do(http.MethodGet, "/api/v1/steop/state/"+sessionID, nil, nil, &out); err != nil {
		return nil, err
	}
	return &out, nil
}

func (c *Client) StatePut(sessionID string, data map[string]interface{}, merge bool) (*State, error) {
	var out State
	body := statePutBody{Data: data, Merge: merge}
	if err := c.do(http.MethodPut, "/api/v1/steop/state/"+sessionID, nil, body, &out); err != nil {
		return nil, err
	}
	return &out, nil
}

func (c *Client) StateDelete(sessionID string) (bool, error) {
	var out stateDeleteResp
	if err := c.do(http.MethodDelete, "/api/v1/steop/state/"+sessionID, nil, nil, &out); err != nil {
		return false, err
	}
	return out.Deleted, nil
}

func (c *Client) CounterIncr(sessionID, name string, delta int64) (int64, error) {
	var out counterResp
	body := counterIncrBody{Counter: name, Delta: delta}
	if err := c.do(http.MethodPost, "/api/v1/steop/state/"+sessionID+"/incr", nil, body, &out); err != nil {
		return 0, err
	}
	return out.Value, nil
}

func (c *Client) CounterReset(sessionID, name string, value int64) (int64, error) {
	var out counterResp
	body := counterResetBody{Counter: name, Value: value}
	if err := c.do(http.MethodPost, "/api/v1/steop/state/"+sessionID+"/reset", nil, body, &out); err != nil {
		return 0, err
	}
	return out.Value, nil
}
