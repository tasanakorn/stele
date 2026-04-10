package client

import (
	"net/http"
	"net/url"
)

type BlobMeta struct {
	Scope     string `json:"scope"`
	Key       string `json:"key"`
	UpdatedAt string `json:"updated_at"`
}

type Blob struct {
	Scope     string `json:"scope"`
	Key       string `json:"key"`
	Content   string `json:"content"`
	CreatedAt string `json:"created_at"`
	UpdatedAt string `json:"updated_at"`
}

type BlobListItem struct {
	Key       string `json:"key"`
	UpdatedAt string `json:"updated_at"`
	Size      int64  `json:"size"`
}

type storagePutBody struct {
	Content string `json:"content"`
}

type storageDeleteResp struct {
	Deleted bool `json:"deleted"`
}

type storageListResp struct {
	Items []BlobListItem `json:"items"`
}

func (c *Client) StoragePut(scope, key, content string) (*BlobMeta, error) {
	q := url.Values{}
	q.Set("scope", scope)
	q.Set("key", key)
	var out BlobMeta
	if err := c.do(http.MethodPut, "/api/v1/steop/storage", q, storagePutBody{Content: content}, &out); err != nil {
		return nil, err
	}
	return &out, nil
}

func (c *Client) StorageGet(scope, key string) (*Blob, error) {
	q := url.Values{}
	q.Set("scope", scope)
	q.Set("key", key)
	var out Blob
	if err := c.do(http.MethodGet, "/api/v1/steop/storage", q, nil, &out); err != nil {
		return nil, err
	}
	return &out, nil
}

func (c *Client) StorageDelete(scope, key string) (bool, error) {
	q := url.Values{}
	q.Set("scope", scope)
	q.Set("key", key)
	var out storageDeleteResp
	if err := c.do(http.MethodDelete, "/api/v1/steop/storage", q, nil, &out); err != nil {
		return false, err
	}
	return out.Deleted, nil
}

func (c *Client) StorageList(scope string) ([]BlobListItem, error) {
	q := url.Values{}
	q.Set("scope", scope)
	var out storageListResp
	if err := c.do(http.MethodGet, "/api/v1/steop/storage/list", q, nil, &out); err != nil {
		return nil, err
	}
	return out.Items, nil
}
