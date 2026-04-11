package client

// MailboxMessage is a message in the steop mailbox.
type MailboxMessage struct {
	ID             int64       `json:"id"`
	FromHost       string      `json:"from_host"`
	FromProjectDir string      `json:"from_project_dir"`
	FromSessionID  string      `json:"from_session_id"`
	ToHost         string      `json:"to_host"`
	ToProjectDir   string      `json:"to_project_dir"`
	ToSessionID    string      `json:"to_session_id"`
	Kind           string      `json:"kind"`
	Subject        string      `json:"subject"`
	Payload        interface{} `json:"payload"`
	CreatedAt      string      `json:"created_at"`
	AckedAt        *string     `json:"acked_at"`
}

// MailboxSend sends a message from a session to a project or session.
// Set toSessionID="" to send to the project level.
func (c *Client) MailboxSend(fromHost, fromProjectDir, fromSessionID, toHost, toProjectDir, toSessionID, kind, subject string, payload interface{}) (int64, error) {
	body := map[string]interface{}{
		"from_host":        fromHost,
		"from_project_dir": fromProjectDir,
		"from_session_id":  fromSessionID,
		"to_host":          toHost,
		"to_project_dir":   toProjectDir,
		"to_session_id":    toSessionID,
		"kind":             kind,
		"subject":          subject,
		"payload":          payload,
	}
	var resp struct {
		ID int64 `json:"id"`
	}
	if err := c.fastClone().rpc("steop.mailbox.send", body, &resp); err != nil {
		return 0, err
	}
	return resp.ID, nil
}

// MailboxSendFromSelf sends a message using the client's own host/projectDir as sender.
// Uses fastClone for fire-and-forget.
func (c *Client) MailboxSendFromSelf(sessionID, toHost, toProjectDir, toSessionID, kind, subject string, payload interface{}) (int64, error) {
	return c.MailboxSend(c.host, c.projectDir, sessionID, toHost, toProjectDir, toSessionID, kind, subject, payload)
}

// MailboxList lists messages for a recipient.
// toSessionID="" returns project-level messages.
func (c *Client) MailboxList(toHost, toProjectDir, toSessionID string, limit int, includeAcked bool) ([]MailboxMessage, error) {
	body := map[string]interface{}{
		"to_host":        toHost,
		"to_project_dir": toProjectDir,
		"to_session_id":  toSessionID,
		"include_acked":  includeAcked,
	}
	if limit > 0 {
		body["limit"] = limit
	}
	var resp struct {
		Messages []MailboxMessage `json:"messages"`
	}
	if err := c.rpc("steop.mailbox.list", body, &resp); err != nil {
		return nil, err
	}
	return resp.Messages, nil
}

// MailboxAck marks a message as acknowledged.
func (c *Client) MailboxAck(id int64) (bool, error) {
	body := map[string]int64{"id": id}
	var resp struct {
		Acked bool `json:"acked"`
	}
	if err := c.rpc("steop.mailbox.ack", body, &resp); err != nil {
		return false, err
	}
	return resp.Acked, nil
}
