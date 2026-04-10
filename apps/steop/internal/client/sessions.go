package client

import (
	"fmt"
	"net/http"
	"net/url"
	"strconv"
)

type SessionSummary struct {
	SessionID   string `json:"session_id"`
	Mode        string `json:"mode"`
	Phase       string `json:"phase"`
	CurrentStep *int64 `json:"current_step,omitempty"`
	TotalSteps  *int64 `json:"total_steps,omitempty"`
	CreatedAt   string `json:"created_at"`
	UpdatedAt   string `json:"updated_at"`
}

type sessionsListResp struct {
	Sessions []SessionSummary `json:"sessions"`
}

type storageScopesResp struct {
	Scopes []string `json:"scopes"`
}

// SessionsList returns up to `limit` sessions ordered by most recently updated.
// Pass limit <= 0 to use server default (100).
func (c *Client) SessionsList(limit int) ([]SessionSummary, error) {
	var q url.Values
	if limit > 0 {
		q = url.Values{}
		q.Set("limit", strconv.Itoa(limit))
	}
	var out sessionsListResp
	if err := c.do(http.MethodGet, "/api/v1/steop/sessions", q, nil, &out); err != nil {
		return nil, err
	}
	return out.Sessions, nil
}

// SessionGet returns the full session state (reuses the State type from state.go).
func (c *Client) SessionGet(id string) (*State, error) {
	if id == "" {
		return nil, fmt.Errorf("session id required")
	}
	var out State
	if err := c.do(http.MethodGet, "/api/v1/steop/sessions/"+id, nil, nil, &out); err != nil {
		return nil, err
	}
	return &out, nil
}

// StorageScopesList returns distinct storage scopes.
func (c *Client) StorageScopesList() ([]string, error) {
	var out storageScopesResp
	if err := c.do(http.MethodGet, "/api/v1/steop/storage/scopes", nil, nil, &out); err != nil {
		return nil, err
	}
	return out.Scopes, nil
}
