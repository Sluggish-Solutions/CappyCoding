package githubclient

import (
	"context"
	"fmt"
	"net/http"
	"net/http/httptest"
	"net/url"
	"strings"
	"testing"
	"time"

	"github.com/google/go-github/v55/github"
)

func TestRepositoryValidate(t *testing.T) {
	t.Parallel()

	cases := []struct {
		name    string
		repo    Repository
		wantErr bool
	}{
		{name: "valid", repo: Repository{Owner: "o", Name: "r"}},
		{name: "missing owner", repo: Repository{Name: "r"}, wantErr: true},
		{name: "missing name", repo: Repository{Owner: "o"}, wantErr: true},
	}

	for _, tc := range cases {
		tc := tc
		t.Run(tc.name, func(t *testing.T) {
			t.Parallel()
			err := tc.repo.Validate()
			if tc.wantErr && err == nil {
				t.Fatalf("expected error")
			}
			if !tc.wantErr && err != nil {
				t.Fatalf("unexpected error: %v", err)
			}
		})
	}
}

func TestPullRequestStatuses(t *testing.T) {
	t.Parallel()

	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path != "/repos/owner/repo/pulls" {
			t.Fatalf("unexpected path: %s", r.URL.Path)
		}
		if got := r.URL.Query().Get("per_page"); got != "5" {
			t.Fatalf("unexpected per_page: %s", got)
		}
		if got := r.URL.Query().Get("state"); got != "closed" {
			t.Fatalf("unexpected state: %s", got)
		}

		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusOK)
		_, _ = w.Write([]byte(`[
{"number":1,"title":"Fix bug","state":"closed","html_url":"https://example.com/pr/1","updated_at":"2024-01-02T15:04:05Z","user":{"login":"alice"}}
]`))
	}))
	defer server.Close()

	client := newTestClient(t, server)

	ctx := context.Background()
	statuses, err := client.PullRequestStatuses(ctx, Repository{Owner: "owner", Name: "repo"}, PullRequestOptions{State: "closed", PerPage: 5})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if len(statuses) != 1 {
		t.Fatalf("expected 1 status, got %d", len(statuses))
	}

	pr := statuses[0]
	if pr.Number != 1 || pr.Author != "alice" {
		t.Fatalf("unexpected pr: %+v", pr)
	}
}

func TestUserPullRequestStatuses(t *testing.T) {
	t.Parallel()

	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path != "/search/issues" {
			t.Fatalf("unexpected path: %s", r.URL.Path)
		}
		if got := r.URL.Query().Get("per_page"); got != "7" {
			t.Fatalf("unexpected per_page: %s", got)
		}
		query := r.URL.Query().Get("q")
		if !strings.Contains(query, "author:alice") || !strings.Contains(query, "is:open") {
			t.Fatalf("unexpected query: %s", query)
		}

		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusOK)
		_, _ = w.Write([]byte(`{"items":[{"number":1,"title":"Feature","state":"open","html_url":"https://example.com/pr/1","updated_at":"2024-01-02T15:04:05Z","user":{"login":"alice"}}]}`))
	}))
	defer server.Close()

	client := newTestClient(t, server)

	prs, err := client.UserPullRequestStatuses(context.Background(), "alice", PullRequestOptions{State: "open", PerPage: 7})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if len(prs) != 1 {
		t.Fatalf("expected 1 pr, got %d", len(prs))
	}

	if prs[0].Author != "alice" || prs[0].State != "open" {
		t.Fatalf("unexpected pr: %+v", prs[0])
	}
}

func TestWorkflowRuns(t *testing.T) {
	t.Parallel()

	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path != "/repos/owner/repo/actions/runs" {
			t.Fatalf("unexpected path: %s", r.URL.Path)
		}
		if got := r.URL.Query().Get("per_page"); got != "3" {
			t.Fatalf("unexpected per_page: %s", got)
		}
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusOK)
		_, _ = w.Write([]byte(`{"total_count":1,"workflow_runs":[{"id":10,"name":"ci","status":"completed","conclusion":"success","html_url":"https://example.com/run","created_at":"2024-01-02T15:04:05Z","updated_at":"2024-01-02T16:04:05Z"}]}`))
	}))
	defer server.Close()

	client := newTestClient(t, server)

	runs, err := client.WorkflowRuns(context.Background(), Repository{Owner: "owner", Name: "repo"}, WorkflowOptions{Branch: "main", PerPage: 3})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if len(runs) != 1 {
		t.Fatalf("expected 1 run, got %d", len(runs))
	}

	if runs[0].Name != "ci" || runs[0].Conclusion != "success" {
		t.Fatalf("unexpected run: %+v", runs[0])
	}
}

func TestUserWorkflowRuns(t *testing.T) {
	t.Parallel()

	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")

		switch r.URL.Path {
		case "/users/alice/repos":
			if got := r.URL.Query().Get("per_page"); got != "50" {
				t.Fatalf("unexpected repo per_page: %s", got)
			}
			_, _ = w.Write([]byte(`[{"name":"repo1","owner":{"login":"alice"}},{"name":"repo2","owner":{"login":"org"}}]`))
		case "/repos/alice/repo1/actions/runs":
			if got := r.URL.Query().Get("branch"); got != "main" {
				t.Fatalf("unexpected branch query: %s", got)
			}
			_, _ = w.Write([]byte(`{"total_count":1,"workflow_runs":[{"id":10,"name":"ci","status":"completed","conclusion":"success","html_url":"https://example.com/run1","created_at":"2024-01-02T15:04:05Z","updated_at":"2024-01-02T16:04:05Z"}]}`))
		case "/repos/org/repo2/actions/runs":
			if got := r.URL.Query().Get("branch"); got != "main" {
				t.Fatalf("unexpected branch query: %s", got)
			}
			_, _ = w.Write([]byte(`{"total_count":1,"workflow_runs":[{"id":11,"name":"deploy","status":"queued","conclusion":"","html_url":"https://example.com/run2","created_at":"2024-01-03T10:00:00Z","updated_at":"2024-01-03T10:05:00Z"}]}`))
		default:
			t.Fatalf("unexpected path: %s", r.URL.Path)
		}
	}))
	defer server.Close()

	client := newTestClient(t, server)

	runs, err := client.UserWorkflowRuns(context.Background(), "alice", WorkflowOptions{Branch: "main", PerPage: 2})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if len(runs) != 2 {
		t.Fatalf("expected 2 runs, got %d", len(runs))
	}

	if runs[0].Name != "ci" || runs[1].Name != "deploy" {
		t.Fatalf("unexpected runs: %+v", runs)
	}
}

func TestCommitCount(t *testing.T) {
	t.Parallel()

	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path != "/repos/owner/repo/commits" {
			t.Fatalf("unexpected path: %s", r.URL.Path)
		}
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusOK)
		_, _ = w.Write([]byte(`[
{"sha":"1","commit":{"author":{"name":"Alice"}},"author":{"login":"alice"}},
{"sha":"2","commit":{"author":{"name":"Bob"}}}
]`))
	}))
	defer server.Close()

	client := newTestClient(t, server)

	metrics, err := client.CommitCount(context.Background(), Repository{Owner: "owner", Name: "repo"}, CommitOptions{})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if metrics.Total != 2 {
		t.Fatalf("expected total 2, got %d", metrics.Total)
	}

	if metrics.ByAuthor["alice"] != 1 || metrics.ByAuthor["Bob"] != 1 {
		t.Fatalf("unexpected metrics: %+v", metrics.ByAuthor)
	}
}

func TestUserCommitCount(t *testing.T) {
	t.Parallel()

	since := time.Date(2024, time.January, 1, 0, 0, 0, 0, time.UTC)
	until := time.Date(2024, time.January, 31, 23, 59, 59, 0, time.UTC)

	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path != "/search/commits" {
			t.Fatalf("unexpected path: %s", r.URL.Path)
		}
		query := r.URL.Query().Get("q")
		// When both since and until are provided, expect range format (YYYY-MM-DD..YYYY-MM-DD)
		expectedRange := fmt.Sprintf("committer-date:%s..%s", since.Format("2006-01-02"), until.Format("2006-01-02"))
		if !strings.Contains(query, "author:alice") || !strings.Contains(query, expectedRange) {
			t.Fatalf("unexpected query: %s (expected to contain: %s)", query, expectedRange)
		}
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusOK)
		_, _ = w.Write([]byte(`{"items":[{"sha":"1","author":{"login":"alice"},"commit":{"author":{"name":"Alice"}}},{"sha":"2","commit":{"author":{"name":"Bob"}}}]}`))
	}))
	defer server.Close()

	client := newTestClient(t, server)

	metrics, err := client.UserCommitCount(context.Background(), "alice", CommitOptions{Since: &since, Until: &until})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if metrics.Total != 2 {
		t.Fatalf("expected total 2, got %d", metrics.Total)
	}

	if metrics.ByAuthor["alice"] != 1 || metrics.ByAuthor["Bob"] != 1 {
		t.Fatalf("unexpected metrics: %+v", metrics.ByAuthor)
	}

	if metrics.Since == nil || !metrics.Since.Equal(since) {
		t.Fatalf("unexpected since: %v", metrics.Since)
	}
	if metrics.Until == nil || !metrics.Until.Equal(until) {
		t.Fatalf("unexpected until: %v", metrics.Until)
	}
}

func newTestClient(t *testing.T, server *httptest.Server) *Client {
	t.Helper()

	api := github.NewClient(server.Client())
	base, err := url.Parse(server.URL + "/")
	if err != nil {
		t.Fatalf("parse base url: %v", err)
	}
	api.BaseURL = base
	api.UploadURL = base

	return NewFromGitHubClient(api)
}

func TestCloneWithToken(t *testing.T) {
	t.Parallel()

	t.Run("reuse base when token empty", func(t *testing.T) {
		t.Parallel()

		base, err := NewClient(context.Background(), WithToken("base-token"))
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}

		clone, err := base.CloneWithToken(context.Background(), "")
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}

		if clone != base {
			t.Fatalf("expected clone to reuse base client")
		}
	})

	t.Run("override token", func(t *testing.T) {
		t.Parallel()

		server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			if got := r.Header.Get("Authorization"); got != "Bearer override-token" {
				t.Fatalf("unexpected authorization header: %s", got)
			}
			w.Header().Set("Content-Type", "application/json")
			_, _ = w.Write([]byte(`[]`))
		}))
		defer server.Close()

		ctx := context.Background()
		base, err := NewClient(ctx, WithToken("base-token"), WithBaseURLs(server.URL, server.URL))
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}

		clone, err := base.CloneWithToken(ctx, "override-token")
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}

		if clone == base {
			t.Fatalf("expected a new client when overriding the token")
		}

		if _, err := clone.PullRequestStatuses(ctx, Repository{Owner: "owner", Name: "repo"}, PullRequestOptions{}); err != nil {
			t.Fatalf("unexpected error calling API: %v", err)
		}
	})
}
