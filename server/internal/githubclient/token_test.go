package githubclient

import (
	"os"
	"path/filepath"
	"testing"
)

func TestLoadTokenFromEnv(t *testing.T) {
	t.Setenv(envTokenKey, "env-token")
	token, err := loadToken()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if token != "env-token" {
		t.Fatalf("unexpected token: %s", token)
	}
}

func TestLoadTokenFromConfig(t *testing.T) {
	t.Setenv(envTokenKey, "")
	dir := t.TempDir()
	t.Setenv("APPDATA", "")
	t.Setenv("XDG_CONFIG_HOME", dir)
	t.Setenv("HOME", dir)

	configDir := filepath.Join(dir, tauriConfigDir)
	if err := os.MkdirAll(configDir, 0o755); err != nil {
		t.Fatalf("mkdir: %v", err)
	}

	configPath := filepath.Join(configDir, "config.json")
	if err := os.WriteFile(configPath, []byte(`{"githubToken":"file-token"}`), 0o600); err != nil {
		t.Fatalf("write config: %v", err)
	}

	token, err := loadToken()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if token != "file-token" {
		t.Fatalf("unexpected token: %s", token)
	}
}

func TestLoadTokenMissing(t *testing.T) {
	t.Setenv(envTokenKey, "")
	t.Setenv("APPDATA", "")
	t.Setenv("XDG_CONFIG_HOME", "")
	t.Setenv("HOME", t.TempDir())

	if _, err := loadToken(); err == nil {
		t.Fatalf("expected error")
	}
}
