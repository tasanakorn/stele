package client

// Blob is a storage value. id is 3-segment for session-scoped blobs or
// 2-segment for project-scoped blobs.
type Blob struct {
	ID        string `json:"id"`
	Key       string `json:"key"`
	Content   string `json:"content"`
	CreatedAt string `json:"created_at"`
	UpdatedAt string `json:"updated_at"`
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

// StoragePut upserts a key-value blob. id may be 2-segment (project scope) or 3-segment (session scope).
func (c *Client) StoragePut(id, key, content string) (*BlobMeta, error) {
	body := map[string]interface{}{
		"id":      id,
		"key":     key,
		"content": content,
	}
	var out BlobMeta
	if err := c.rpc("steop.storage.put", body, &out); err != nil {
		return nil, err
	}
	return &out, nil
}

// StorageGet retrieves a blob.
func (c *Client) StorageGet(id, key string) (*Blob, error) {
	body := map[string]interface{}{"id": id, "key": key}
	var out Blob
	if err := c.rpc("steop.storage.get", body, &out); err != nil {
		return nil, err
	}
	return &out, nil
}

// StorageDelete deletes a blob.
func (c *Client) StorageDelete(id, key string) (bool, error) {
	body := map[string]interface{}{"id": id, "key": key}
	var resp struct {
		Deleted bool `json:"deleted"`
	}
	if err := c.rpc("steop.storage.delete", body, &resp); err != nil {
		return false, err
	}
	return resp.Deleted, nil
}

// StorageList lists keys for a storage scope.
func (c *Client) StorageList(id string) ([]BlobListItem, error) {
	body := map[string]interface{}{"id": id}
	var resp struct {
		Items []BlobListItem `json:"items"`
	}
	if err := c.rpc("steop.storage.list", body, &resp); err != nil {
		return nil, err
	}
	return resp.Items, nil
}
