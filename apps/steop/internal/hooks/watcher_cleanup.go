package hooks

import (
	"encoding/json"

	"github.com/tasanakorn/stele/apps/steop/internal/client"
	"github.com/tasanakorn/stele/apps/steop/internal/logging"
)

type activeTask struct {
	TaskID           string `json:"task_id"`
	RequestMessageID int64  `json:"request_message_id"`
	From             string `json:"from"`
}

// cleanupWatcherTasks sends TASK:FAILED for any tasks the watcher had claimed
// but not completed. Best-effort: errors are logged but never propagated.
func cleanupWatcherTasks(c *client.Client, sid string) {
	blob, err := c.StorageGet(sid, "watcher:active_tasks")
	if err != nil {
		logging.Debugf("watcher cleanup: storage get: %v", err)
		return
	}
	if blob.Content == "" {
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

	if _, err := c.StorageDelete(sid, "watcher:active_tasks"); err != nil {
		logging.Debugf("watcher cleanup: storage delete: %v", err)
	}
}
