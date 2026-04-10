package client

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"strings"
	"time"

	"github.com/tasanakorn/stele/apps/steop/internal/config"
)

// ErrNotFound is returned when the server responds with HTTP 404.
var ErrNotFound = errors.New("not found")

type Client struct {
	baseURL string
	authKey string
	http    *http.Client
}

// New constructs a Client with the given base URL and auth key.
func New(baseURL, authKey string) *Client {
	return &Client{
		baseURL: strings.TrimRight(baseURL, "/"),
		authKey: authKey,
		http: &http.Client{
			Timeout: 10 * time.Second,
		},
	}
}

// NewFromConfig loads the config and returns a client for the active profile.
func NewFromConfig() (*Client, error) {
	c, err := config.Load()
	if err != nil {
		return nil, err
	}
	p, err := c.Active()
	if err != nil {
		return nil, err
	}
	return New(p.ServerURL, p.AuthKey), nil
}

type errorBody struct {
	Error string `json:"error"`
}

// do executes an HTTP request with JSON body. If body is nil, no body is sent.
// On non-2xx responses, decodes an error body and returns an error. 404 returns
// ErrNotFound.
func (c *Client) do(method, path string, query url.Values, body interface{}, out interface{}) error {
	u := c.baseURL + path
	if len(query) > 0 {
		u += "?" + query.Encode()
	}

	var reader io.Reader
	if body != nil {
		buf, err := json.Marshal(body)
		if err != nil {
			return fmt.Errorf("marshal body: %w", err)
		}
		reader = bytes.NewReader(buf)
	}

	req, err := http.NewRequest(method, u, reader)
	if err != nil {
		return fmt.Errorf("new request: %w", err)
	}
	if body != nil {
		req.Header.Set("Content-Type", "application/json")
	}
	req.Header.Set("Accept", "application/json")
	if c.authKey != "" {
		req.Header.Set("X-Stele-Key", c.authKey)
	}

	resp, err := c.http.Do(req)
	if err != nil {
		return fmt.Errorf("http do: %w", err)
	}
	defer resp.Body.Close()

	data, err := io.ReadAll(resp.Body)
	if err != nil {
		return fmt.Errorf("read body: %w", err)
	}

	if resp.StatusCode == http.StatusNotFound {
		return ErrNotFound
	}
	if resp.StatusCode < 200 || resp.StatusCode >= 300 {
		var eb errorBody
		if err := json.Unmarshal(data, &eb); err == nil && eb.Error != "" {
			return fmt.Errorf("http %d: %s", resp.StatusCode, eb.Error)
		}
		return fmt.Errorf("http %d: %s", resp.StatusCode, strings.TrimSpace(string(data)))
	}

	if out != nil && len(data) > 0 {
		if err := json.Unmarshal(data, out); err != nil {
			return fmt.Errorf("decode response: %w", err)
		}
	}
	return nil
}
