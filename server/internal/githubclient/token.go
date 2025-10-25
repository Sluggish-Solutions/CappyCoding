package githubclient

import (
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
)

const (
	envTokenKey    = "GITHUB_TOKEN"
	tauriConfigDir = "com.sluggish-solutions.capycoding"
)

// ErrMissingToken is returned when no GitHub token could be discovered.
var ErrMissingToken = errors.New("github token not found in environment or tauri config")

type tauriConfig struct {
	GitHubToken string `json:"githubToken"`
	GitHub      struct {
		Token string `json:"token"`
	} `json:"github"`
}

func loadToken() (string, error) {
	if token := os.Getenv(envTokenKey); token != "" {
		return token, nil
	}

	configPaths := []string{
		filepath.Join(os.Getenv("APPDATA"), tauriConfigDir, "config.json"),
		filepath.Join(os.Getenv("XDG_CONFIG_HOME"), tauriConfigDir, "config.json"),
		filepath.Join(os.Getenv("HOME"), ".config", tauriConfigDir, "config.json"),
	}

	for _, path := range configPaths {
		if path == "" {
			continue
		}

		file, err := os.Open(path)
		if err != nil {
			continue
		}

		content, err := io.ReadAll(file)
		file.Close()
		if err != nil {
			continue
		}

		var cfg tauriConfig
		if err := json.Unmarshal(content, &cfg); err != nil {
			continue
		}

		if cfg.GitHubToken != "" {
			return cfg.GitHubToken, nil
		}

		if cfg.GitHub.Token != "" {
			return cfg.GitHub.Token, nil
		}
	}

	return "", ErrMissingToken
}

// TokenDiagnostic returns an error explaining the token discovery rules.
func TokenDiagnostic() error {
	_, err := loadToken()
	if err == nil {
		return nil
	}

	return fmt.Errorf("%w: set %s or create a config.json with a githubToken field", err, envTokenKey)
}
