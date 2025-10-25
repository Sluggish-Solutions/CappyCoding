package http

import (
	"context"
	"encoding/json"
	"errors"
	nethttp "net/http"
	"net/http/httptest"
	"sync"
	"testing"

	"github.com/google/go-github/v55/github"
	"github.com/labstack/echo/v4"

	"cappycoding/server/internal/githubclient"
)

func TestExtractGitHubToken(t *testing.T) {
	t.Parallel()

	cases := []struct {
		name   string
		header map[string]string
		want   string
	}{
		{
			name: "token scheme",
			header: map[string]string{
				"Authorization": "token abc123",
			},
			want: "abc123",
		},
		{
			name: "bearer scheme",
			header: map[string]string{
				"Authorization": "Bearer def456",
			},
			want: "def456",
		},
		{
			name: "custom header",
			header: map[string]string{
				"X-GitHub-Token": "xyz",
			},
			want: "xyz",
		},
		{
			name:   "missing headers",
			header: map[string]string{},
			want:   "",
		},
	}

	for _, tc := range cases {
		tc := tc
		t.Run(tc.name, func(t *testing.T) {
			t.Parallel()

			e := echo.New()
			req := httptest.NewRequest(nethttp.MethodGet, "/", nil)
			for k, v := range tc.header {
				req.Header.Set(k, v)
			}
			rec := httptest.NewRecorder()
			c := e.NewContext(req, rec)

			if got := extractGitHubToken(c); got != tc.want {
				t.Fatalf("unexpected token: %q", got)
			}
		})
	}
}

func TestResolveClient(t *testing.T) {
	t.Parallel()

	ctx := context.Background()

	t.Run("uses base client when no override", func(t *testing.T) {
		t.Parallel()

		base := githubclient.NewFromGitHubClient(github.NewClient(nil))

		resolved, err := resolveClient(ctx, base, "")
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}

		if resolved != base {
			t.Fatalf("expected base client to be reused")
		}
	})

	t.Run("creates new client when base missing", func(t *testing.T) {
		t.Parallel()

		resolved, err := resolveClient(ctx, nil, "token")
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}

		if resolved == nil {
			t.Fatalf("expected client instance")
		}
	})

	t.Run("errors when no token available", func(t *testing.T) {
		t.Parallel()

		if _, err := resolveClient(ctx, nil, ""); err == nil {
			t.Fatalf("expected error")
		} else if !errors.Is(err, githubclient.ErrMissingToken) {
			t.Fatalf("unexpected error: %v", err)
		}
	})
}

func TestRegisterRoutes_HeaderOverrideWithoutBaseClient(t *testing.T) {
	// this test mutates package level defaults so it cannot run in parallel

	originalBaseURL := defaultBaseURL
	originalUploadURL := defaultUploadURL
	defer func() {
		defaultBaseURL = originalBaseURL
		defaultUploadURL = originalUploadURL
	}()

	const token = "override"

	var (
		observedPath string
		observedAuth string
		mu           sync.Mutex
	)

	ts := httptest.NewServer(nethttp.HandlerFunc(func(w nethttp.ResponseWriter, r *nethttp.Request) {
		mu.Lock()
		observedPath = r.URL.Path
		observedAuth = r.Header.Get("Authorization")
		mu.Unlock()

		w.Header().Set("Content-Type", "application/json")
		if err := json.NewEncoder(w).Encode([]map[string]any{
			{
				"number":     1,
				"title":      "Example",
				"state":      "open",
				"html_url":   "https://example.com/pr/1",
				"updated_at": "2024-01-02T03:04:05Z",
				"user": map[string]any{
					"login": "octocat",
				},
			},
		}); err != nil {
			t.Fatalf("failed to write response: %v", err)
		}
	}))
	defer ts.Close()

	defaultBaseURL = ts.URL + "/"
	defaultUploadURL = ts.URL + "/"

	e := echo.New()
	RegisterRoutes(e, nil)

	req := httptest.NewRequest(nethttp.MethodGet, "/metrics/prs?owner=owner&repo=repo", nil)
	req.Header.Set("Authorization", "token "+token)
	rec := httptest.NewRecorder()

	e.ServeHTTP(rec, req)

	if rec.Code != nethttp.StatusOK {
		t.Fatalf("unexpected status: %d body=%s", rec.Code, rec.Body.String())
	}

	var payload []githubclient.PRStatus
	if err := json.NewDecoder(rec.Body).Decode(&payload); err != nil {
		t.Fatalf("failed to decode response: %v", err)
	}

	if len(payload) != 1 {
		t.Fatalf("unexpected payload length: %d", len(payload))
	}

	if payload[0].Author != "octocat" {
		t.Fatalf("unexpected author: %s", payload[0].Author)
	}

	mu.Lock()
	path := observedPath
	auth := observedAuth
	mu.Unlock()

	if path != "/repos/owner/repo/pulls" {
		t.Fatalf("unexpected path: %s", path)
	}

	if auth != "Bearer "+token {
		t.Fatalf("unexpected authorization header: %q", auth)
	}
}
