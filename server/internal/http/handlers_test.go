package http

import (
	"context"
	"errors"
	nethttp "net/http"
	"net/http/httptest"
	"strings"
	"testing"
	"time"

	"github.com/google/go-github/v55/github"
	"github.com/labstack/echo/v4"

	"cappycoding/server/internal/claude"
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

func TestRegisterRoutesWithoutBaseClient(t *testing.T) {
	server := httptest.NewServer(nethttp.HandlerFunc(func(w nethttp.ResponseWriter, r *nethttp.Request) {
		if r.URL.Path != "/search/issues" {
			t.Fatalf("unexpected path: %s", r.URL.Path)
		}
		if got := r.Header.Get("Authorization"); got != "Bearer override-token" {
			t.Fatalf("unexpected authorization header: %s", got)
		}
		query := r.URL.Query().Get("q")
		if !strings.Contains(query, "author:alice") {
			t.Fatalf("unexpected query: %s", query)
		}

		w.Header().Set("Content-Type", "application/json")
		_, _ = w.Write([]byte(`{"items":[{"number":1,"title":"Add feature","state":"open","html_url":"https://example.com/pr/1","updated_at":"2024-01-02T15:04:05Z","user":{"login":"alice"}}]}`))
	}))
	defer server.Close()

	originalFactory := newGitHubClient
	newGitHubClient = func(ctx context.Context, opts ...githubclient.ClientOption) (*githubclient.Client, error) {
		opts = append(opts, githubclient.WithBaseURLs(server.URL, server.URL), githubclient.WithHTTPClient(server.Client()))
		return githubclient.NewClient(ctx, opts...)
	}
	defer func() { newGitHubClient = originalFactory }()

	e := echo.New()
	RegisterRoutes(e, nil, claude.NewStore(10))

	req := httptest.NewRequest(nethttp.MethodGet, "/metrics/prs?user=alice", nil)
	rec := httptest.NewRecorder()
	e.ServeHTTP(rec, req)

	if rec.Code != nethttp.StatusUnauthorized {
		t.Fatalf("expected unauthorized when token missing, got %d", rec.Code)
	}

	req = httptest.NewRequest(nethttp.MethodGet, "/metrics/prs?user=alice", nil)
	req.Header.Set("Authorization", "Bearer override-token")
	rec = httptest.NewRecorder()
	e.ServeHTTP(rec, req)

	if rec.Code != nethttp.StatusOK {
		t.Fatalf("expected success, got %d with body %s", rec.Code, rec.Body.String())
	}

	if body := rec.Body.String(); !strings.Contains(body, "Add feature") {
		t.Fatalf("unexpected body: %s", body)
	}
}

func TestConvertClaudePayload(t *testing.T) {
	t.Parallel()

	now := time.Now().UTC().Truncate(time.Second)
	payload := claudeMetricsPayload{
		Timestamp:           now.Format(time.RFC3339),
		WindowHours:         2,
		BurnRatePerHour:     1.23,
		TotalCostUSD:        4.56,
		InputTokens:         100,
		OutputTokens:        200,
		CacheCreationTokens: 10,
		CacheReadTokens:     20,
		TotalTokens:         330,
		SessionCount:        5,
		ActiveSessionID:     "abc",
		LastActivity:        now.Format(time.RFC3339),
		Source:              "local",
	}

	metrics, err := convertClaudePayload(payload)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if metrics.TotalTokens != payload.TotalTokens {
		t.Fatalf("unexpected metrics: %+v", metrics)
	}
}

func TestClaudeMetricsEndpoints(t *testing.T) {
	t.Parallel()

	store := claude.NewStore(5)
	e := echo.New()
	RegisterRoutes(e, nil, store)

	req := httptest.NewRequest(nethttp.MethodGet, "/metrics/claude", nil)
	rec := httptest.NewRecorder()
	e.ServeHTTP(rec, req)

	if rec.Code != nethttp.StatusNotFound {
		t.Fatalf("expected 404 when no metrics available, got %d", rec.Code)
	}

	now := time.Now().UTC().Truncate(time.Second)
	body := strings.NewReader(`{
                "timestamp": "` + now.Format(time.RFC3339) + `",
                "window_hours": 1,
                "burn_rate_per_hour": 2.5,
                "total_cost_usd": 5.0,
                "input_tokens": 100,
                "output_tokens": 200,
                "cache_creation_tokens": 10,
                "cache_read_tokens": 20,
                "total_tokens": 330,
                "session_count": 4,
                "active_session_id": "session-1",
                "last_activity": "` + now.Format(time.RFC3339) + `",
                "source": "test"
        }`)

	req = httptest.NewRequest(nethttp.MethodPost, "/metrics/claude", body)
	req.Header.Set("Content-Type", "application/json")
	rec = httptest.NewRecorder()
	e.ServeHTTP(rec, req)

	if rec.Code != nethttp.StatusOK {
		t.Fatalf("expected success when posting metrics, got %d: %s", rec.Code, rec.Body.String())
	}

	req = httptest.NewRequest(nethttp.MethodGet, "/metrics/claude", nil)
	rec = httptest.NewRecorder()
	e.ServeHTTP(rec, req)

	if rec.Code != nethttp.StatusOK {
		t.Fatalf("expected success fetching metrics, got %d", rec.Code)
	}
}
