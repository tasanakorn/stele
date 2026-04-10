package client

import "net/http"

type NotifyRequest struct {
	Title    string `json:"title,omitempty"`
	Body     string `json:"body,omitempty"`
	Subtitle string `json:"subtitle,omitempty"`
	Sound    bool   `json:"sound,omitempty"`
}

func (c *Client) Notify(req NotifyRequest) error {
	return c.do(http.MethodPost, "/api/v1/steop/notify", nil, &req, nil)
}
