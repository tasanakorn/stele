package client

type NotifyRequest struct {
	Title    string `json:"title,omitempty"`
	Body     string `json:"body,omitempty"`
	Subtitle string `json:"subtitle,omitempty"`
	Sound    bool   `json:"sound,omitempty"`
}

func (c *Client) Notify(req NotifyRequest) error {
	return c.rpc("steop.notify", req, nil)
}
