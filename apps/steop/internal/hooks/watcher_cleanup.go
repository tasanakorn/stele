package hooks

import (
	"context"
	"encoding/json"

	"github.com/tasanakorn/stele/apps/steop/internal/client"
	"github.com/tasanakorn/stele/apps/steop/internal/logging"
	"github.com/tasanakorn/stele/apps/steop/internal/store"
)

type activeTask struct {
	TaskID           string `json:"task_id"`
	RequestMessageID int64  `json:"request_message_id"`
	From             string `json:"from"`
}

// cleanupWatcherTasks sends TASK:FAILED for any tasks the watcher had claimed
// but not completed. Reads watcher:active_tasks from the local store, sends
// mailbox notifications via the HTTP client, and deletes the local blob.
// Best-effort: errors are logged but never propagated.
func cleanupWatcherTasks(db *store.DB, c *client.Client, id store.Identity, sid string) {
	if db == nil || c == nil {
		return
	}
	ctx := context.Background()
	blob, err := db.StorageGet(ctx, id, "watcher:active_tasks")
	if err != nil {
		logging.Debugf("watcher cleanup: storage get: %v", err)
		return
	}
	if blob == nil || blob.Content == "" {
		return
	}

	var tasks []activeTask
	if err := json.Unmarshal([]byte(blob.Content), &tasks); err != nil {
		logging.Debugf("watcher cleanup: parse tasks: %v", err)
		return
	}
	if len(tasks) == 0 {
		return
	}

	for _, t := range tasks {
		if _, err := c.MailboxSend(sid, t.From, client.MailboxSendOptions{
			MessageType: "TASK:FAILED",
			Subject:     "Session ended before task completed",
			Meta: map[string]interface{}{
				"task_id":            t.TaskID,
				"request_message_id": t.RequestMessageID,
				"error":              "session ended before task completed",
			},
		}); err != nil {
			logging.Debugf("watcher cleanup: send TASK:FAILED for %s: %v", t.TaskID, err)
		}
	}

	if _, err := db.StorageDelete(ctx, id, "watcher:active_tasks"); err != nil {
		logging.Debugf("watcher cleanup: storage delete: %v", err)
	}
}
