package http

import (
	"context"
	"errors"
	nethttp "net/http"
	"net/http/httptest"
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
