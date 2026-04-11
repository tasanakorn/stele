package client

// Blob is a storage value with full identity.
type Blob struct {
	Host       string  `json:"host"`
	ProjectDir string  `json:"project_dir"`
	SessionID  *string `json:"session_id"`
	Key        string  `json:"key"`
	Content    string  `json:"content"`
	CreatedAt  string  `json:"created_at"`
	UpdatedAt  string  `json:"updated_at"`
}

// BlobMeta is returned by put operations.
type BlobMeta struct {
	Key       string `json:"key"`
	UpdatedAt string `json:"updated_at"`
}

// BlobListItem is a single entry in a storage list.
type BlobListItem struct {
	Key       string `json:"key"`
	UpdatedAt string `json:"updated_at"`
	Size      int64  `json:"size"`
}

func storageBody(host, projectDir, sessionID string) map[string]interface{} {
	body := map[string]interface{}{
		"host":        host,
		"project_dir": projectDir,
	}
	if sessionID != "" {
		body["session_id"] = sessionID
	}
	return body
}

// StoragePut upserts a key-value blob. sessionID="" routes to project scope.
func (c *Client) StoragePut(host, projectDir, sessionID, key, content string) (*BlobMeta, error) {
	body := storageBody(host, projectDir, sessionID)
	body["key"] = key
	body["content"] = content
	var out BlobMeta
	if err := c.rpc("steop.storage.put", body, &out); err != nil {
		return nil, err
	}
	return &out, nil
}

// StorageGet retrieves a blob. sessionID="" routes to project scope.
func (c *Client) StorageGet(host, projectDir, sessionID, key string) (*Blob, error) {
	body := storageBody(host, projectDir, sessionID)
	body["key"] = key
	var out Blob
	if err := c.rpc("steop.storage.get", body, &out); err != nil {
		return nil, err
	}
	return &out, nil
}

// StorageDelete deletes a blob. sessionID="" routes to project scope.
func (c *Client) StorageDelete(host, projectDir, sessionID, key string) (bool, error) {
	body := storageBody(host, projectDir, sessionID)
	body["key"] = key
	var resp struct {
		Deleted bool `json:"deleted"`
	}
	if err := c.rpc("steop.storage.delete", body, &resp); err != nil {
		return false, err
	}
	return resp.Deleted, nil
}

// StorageList lists keys for a storage scope. sessionID="" routes to project scope.
func (c *Client) StorageList(host, projectDir, sessionID string) ([]BlobListItem, error) {
	body := storageBody(host, projectDir, sessionID)
	var resp struct {
		Items []BlobListItem `json:"items"`
	}
	if err := c.rpc("steop.storage.list", body, &resp); err != nil {
		return nil, err
	}
	return resp.Items, nil
}
