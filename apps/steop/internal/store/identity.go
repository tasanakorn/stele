package store

import (
	"errors"
	"regexp"
	"strings"
)

var uuidRe = regexp.MustCompile(`^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$`)

// Identity is the parsed composite id. SessionID is empty for project-level
// ids, "USER" for user-level ids, or a canonical UUID string for session-level
// ids.
type Identity struct {
	Host       string
	ProjectDir string
	SessionID  string
}

// IsProject reports whether the identity is 2-segment (project-level).
func (i Identity) IsProject() bool { return i.SessionID == "" }

// ParseID parses the SSH/SCP-style composite id per docs/steop/DESIGN.md §4.
//   - host is the segment before the first ':'.
//   - If no further ':' in the remainder, the id is project-level.
//   - Otherwise the last ':' splits project_dir from the tail. Tail must be
//     either a canonical 8-4-4-4-12 UUID (lowercase hex) or the literal
//     string "USER".
func ParseID(s string) (Identity, error) {
	if s == "" {
		return Identity{}, errors.New("empty id")
	}
	firstColon := strings.IndexByte(s, ':')
	if firstColon < 0 {
		return Identity{}, errors.New("id must contain at least one ':'")
	}
	host := s[:firstColon]
	if host == "" {
		return Identity{}, errors.New("id host segment is empty")
	}
	remainder := s[firstColon+1:]
	lastColon := strings.LastIndexByte(remainder, ':')
	if lastColon < 0 {
		// 2-segment: project-level.
		if remainder == "" {
			return Identity{}, errors.New("id project_dir segment is empty")
		}
		return Identity{Host: host, ProjectDir: remainder}, nil
	}
	projectDir := remainder[:lastColon]
	tail := remainder[lastColon+1:]
	if projectDir == "" {
		return Identity{}, errors.New("id project_dir segment is empty")
	}
	if tail == "USER" {
		return Identity{Host: host, ProjectDir: projectDir, SessionID: "USER"}, nil
	}
	if uuidRe.MatchString(tail) {
		return Identity{Host: host, ProjectDir: projectDir, SessionID: tail}, nil
	}
	return Identity{}, errors.New("id 3rd segment must be a session UUID or the literal 'USER'")
}
