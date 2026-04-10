package config

import (
	"fmt"
	"os"
	"path/filepath"
	"runtime"

	"github.com/BurntSushi/toml"
)

type Config struct {
	DefaultProfile string             `toml:"default_profile"`
	Profiles       map[string]Profile `toml:"profiles"`
}

type Profile struct {
	ServerURL string `toml:"server_url"`
	AuthKey   string `toml:"auth_key"`
}

const defaultServerURL = "http://127.0.0.1:3100"

// configPaths returns the list of paths to check for config, in priority order.
func configPaths() []string {
	var paths []string
	home, err := os.UserHomeDir()
	if err != nil {
		return paths
	}
	// Primary: ~/.config/stele/config.toml
	paths = append(paths, filepath.Join(home, ".config", "stele", "config.toml"))
	// macOS fallback: ~/Library/Application Support/stele/config.toml
	if runtime.GOOS == "darwin" {
		paths = append(paths, filepath.Join(home, "Library", "Application Support", "stele", "config.toml"))
	}
	return paths
}

// Load reads the config from disk. If no config file exists, returns an empty
// config (not an error). The binary must never panic on missing config.
func Load() (*Config, error) {
	c := &Config{
		Profiles: make(map[string]Profile),
	}
	for _, p := range configPaths() {
		data, err := os.ReadFile(p)
		if err != nil {
			if os.IsNotExist(err) {
				continue
			}
			return c, fmt.Errorf("read config %s: %w", p, err)
		}
		if err := toml.Unmarshal(data, c); err != nil {
			return c, fmt.Errorf("parse config %s: %w", p, err)
		}
		return c, nil
	}
	// No config found — return empty, usable Config.
	return c, nil
}

// Active returns the currently active profile. STELE_URL / STELE_AUTH_KEY
// environment variables override any on-disk value. If no config is present,
// a default local profile is returned.
func (c *Config) Active() (*Profile, error) {
	var p Profile

	if c != nil && c.DefaultProfile != "" {
		if found, ok := c.Profiles[c.DefaultProfile]; ok {
			p = found
		}
	} else if c != nil && len(c.Profiles) > 0 {
		// If no default profile set, try "local" then any first profile.
		if found, ok := c.Profiles["local"]; ok {
			p = found
		} else {
			for _, v := range c.Profiles {
				p = v
				break
			}
		}
	}

	if p.ServerURL == "" {
		p.ServerURL = defaultServerURL
	}

	if v := os.Getenv("STELE_URL"); v != "" {
		p.ServerURL = v
	}
	if v := os.Getenv("STELE_AUTH_KEY"); v != "" {
		p.AuthKey = v
	}

	return &p, nil
}
