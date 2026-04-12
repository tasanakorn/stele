package client

// MailboxMessage is a v0.8 mailbox row. From and To are composite identifiers
// (2- or 3-segment, see docs/steop/DESIGN.md §4). Status is one of
// "NEW" | "READ" | "ARCHIVE".
type MailboxMessage struct {
	MessageID   int64       `json:"message_id"`
	From        string      `json:"from"`
	To          string      `json:"to"`
	Subject     string      `json:"subject"`
	MessageType string      `json:"message_type"`
	Meta        interface{} `json:"meta"`
	Payload     interface{} `json:"payload"`
	CreatedAt   string      `json:"created_at"`
	Status      string      `json:"status"`
}

// MailboxSendOptions carries the optional fields of a mailbox.send call.
// Zero values are sent as server defaults (empty subject, message_type="NOTE",
// empty meta/payload objects).
type MailboxSendOptions struct {
	// From overrides the implicit sender derivation. Leave empty to let the
	// server derive from the caller's id.
	From        string
	Subject     string
	MessageType string
	Meta        interface{}
	Payload     interface{}
}

// MailboxSend sends a message. The caller's own composite id (`id`) is used to
// derive the implicit `from` unless opts.From is set. `to` is any 2-seg or
// 3-seg composite id. Uses fastClone for fire-and-forget.
func (c *Client) MailboxSend(id, to string, opts MailboxSendOptions) (*MailboxMessage, error) {
	body := map[string]interface{}{
		"id": id,
		"to": to,
	}
	if opts.From != "" {
		body["from"] = opts.From
	}
	if opts.Subject != "" {
		body["subject"] = opts.Subject
	}
	if opts.MessageType != "" {
		body["message_type"] = opts.MessageType
	}
	if opts.Meta != nil {
		body["meta"] = opts.Meta
	}
	if opts.Payload != nil {
		body["payload"] = opts.Payload
	}
	var resp MailboxMessage
	if err := c.fastClone().rpc("steop.mailbox.send", body, &resp); err != nil {
		return nil, err
	}
	return &resp, nil
}

// MailboxListOptions carries the optional fields of a mailbox.list call.
type MailboxListOptions struct {
	// To overrides the implicit recipient filter. Leave empty to default to
	// the caller's own `id`.
	To          string
	Status      []string // default ["NEW"]
	MessageType string
	Limit       int
}

// MailboxList queries the mailbox. The caller's own `id` is passed as the
// identity and (unless opts.To is set) as the implicit recipient filter.
func (c *Client) MailboxList(id string, opts MailboxListOptions) ([]MailboxMessage, error) {
	body := map[string]interface{}{"id": id}
	if opts.To != "" {
		body["to"] = opts.To
	}
	if len(opts.Status) > 0 {
		body["status"] = opts.Status
	}
	if opts.MessageType != "" {
		body["message_type"] = opts.MessageType
	}
	if opts.Limit > 0 {
		body["limit"] = opts.Limit
	}
	var resp struct {
		Messages []MailboxMessage `json:"messages"`
	}
	if err := c.rpc("steop.mailbox.list", body, &resp); err != nil {
		return nil, err
	}
	return resp.Messages, nil
}

// MailboxGet fetches a single message by row id. Side-effect free.
func (c *Client) MailboxGet(id string, messageID int64) (*MailboxMessage, error) {
	body := map[string]interface{}{"id": id, "message_id": messageID}
	var resp MailboxMessage
	if err := c.rpc("steop.mailbox.get", body, &resp); err != nil {
		return nil, err
	}
	return &resp, nil
}

// MailboxRead marks a NEW message as READ. Returns an error if the message is
// not in NEW state (server returns 409).
func (c *Client) MailboxRead(id string, messageID int64) error {
	body := map[string]interface{}{"id": id, "message_id": messageID}
	var resp struct {
		MessageID int64  `json:"message_id"`
		Status    string `json:"status"`
	}
	return c.rpc("steop.mailbox.read", body, &resp)
}

// MailboxArchive archives a message. Legal from NEW or READ. Returns an error
// if the message is already ARCHIVE (server returns 409).
func (c *Client) MailboxArchive(id string, messageID int64) error {
	body := map[string]interface{}{"id": id, "message_id": messageID}
	var resp struct {
		MessageID int64  `json:"message_id"`
		Status    string `json:"status"`
	}
	return c.rpc("steop.mailbox.archive", body, &resp)
}
