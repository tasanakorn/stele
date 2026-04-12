package client

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"os"
	"strings"
	"time"
	"unicode"

	"github.com/tasanakorn/stele/apps/steop/internal/config"
)

// ErrNotFound is returned when the server responds with HTTP 404.
var ErrNotFound = errors.New("not found")

type Client struct {
	baseURL    string
	authKey    string
	http       *http.Client
	host       string
	projectDir string
}

// New constructs a Client with the given base URL and auth key. Host and
// project_dir are auto-detected from the environment so every request carries
// composite identity headers by default, regardless of whether a config file
// exists. NewFromConfig overrides host from the active profile when set.
func New(baseURL, authKey string) *Client {
	return &Client{
		baseURL:    strings.TrimRight(baseURL, "/"),
		authKey:    authKey,
		host:       detectHost(),
		projectDir: detectProjectDir(),
		http: &http.Client{
			Timeout: 10 * time.Second,
		},
	}
}

// NewFromConfig loads the config and returns a client for the active profile.
// If the profile carries an explicit Host, it overrides the auto-detected one;
// otherwise the auto-detected hostname from New() is kept.
func NewFromConfig() (*Client, error) {
	c, err := config.Load()
	if err != nil {
		return nil, err
	}
	p, err := c.Active()
	if err != nil {
		return nil, err
	}
	client := New(p.ServerURL, p.AuthKey)
	if p.Host != "" {
		client.host = sanitizeHeader(p.Host)
	}
	return client, nil
}

// detectHost returns the sanitized host identifier for this machine. Precedence:
// STELE_HOST env var, then os.Hostname(). Returns empty string on failure.
func detectHost() string {
	if v := os.Getenv("STELE_HOST"); v != "" {
		return sanitizeHeader(v)
	}
	h, err := os.Hostname()
	if err != nil {
		return ""
	}
	return sanitizeHeader(h)
}

// detectProjectDir returns the sanitized project directory. Precedence:
// CLAUDE_PROJECT_DIR env var, PWD env var, os.Getwd(). Returns empty string
// if nothing resolves.
func detectProjectDir() string {
	if v := os.Getenv("CLAUDE_PROJECT_DIR"); v != "" {
		return sanitizeHeader(v)
	}
	if v := os.Getenv("PWD"); v != "" {
		return sanitizeHeader(v)
	}
	if wd, err := os.Getwd(); err == nil {
		return sanitizeHeader(wd)
	}
	return ""
}

// sanitizeHeader strips non-printable characters so the result is safe to
// set as an HTTP header value. Keeps ASCII-graphic runes plus '/' (for paths),
// but always removes ':' so the value can be safely used as a colon-separated
// segment in a composite steop id.
func sanitizeHeader(s string) string {
	var b strings.Builder
	b.Grow(len(s))
	for _, r := range s {
		if r == ':' {
			b.WriteRune('-')
			continue
		}
		if r == '/' || (r < unicode.MaxASCII && unicode.IsGraphic(r) && r != ' ') {
			b.WriteRune(r)
		}
	}
	return b.String()
}

// ComposeProjectID builds a 2-segment steop id "host:project_dir".
func ComposeProjectID(host, projectDir string) string {
	return host + ":" + projectDir
}

// ComposeSessionID builds a 3-segment steop id "host:project_dir:session_id".
func ComposeSessionID(host, projectDir, sessionID string) string {
	return host + ":" + projectDir + ":" + sessionID
}

// ProjectID returns the client's 2-segment composite project id.
func (c *Client) ProjectID() string {
	return ComposeProjectID(c.host, c.projectDir)
}

// SessionCompositeID returns the 3-segment composite session id for this client.
func (c *Client) SessionCompositeID(sessionID string) string {
	return ComposeSessionID(c.host, c.projectDir, sessionID)
}

// WithRequestContext returns a shallow copy of the client with host and
// project_dir overrides applied for request-scoped headers.
func (c *Client) WithRequestContext(host, projectDir string) *Client {
	clone := *c
	if host != "" {
		clone.host = host
	}
	if projectDir != "" {
		clone.projectDir = projectDir
	}
	return &clone
}

// fastClone returns a shallow copy with a very short HTTP timeout suitable for
// fire-and-forget best-effort POSTs.
func (c *Client) fastClone() *Client {
	clone := *c
	clone.http = &http.Client{Timeout: 500 * time.Millisecond}
	return &clone
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

// rpc posts to /api/v1/steop/<method> with a JSON body.
func (c *Client) rpc(method string, body any, out any) error {
	return c.do("POST", "/api/v1/steop/"+method, nil, body, out)
}

// Host returns the client's resolved host name.
func (c *Client) Host() string { return c.host }

// ProjectDir returns the client's resolved project directory.
func (c *Client) ProjectDir() string { return c.projectDir }
