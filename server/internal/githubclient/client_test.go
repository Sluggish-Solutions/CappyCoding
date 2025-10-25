package githubclient

import (
	"context"
	"net/http"
	"net/http/httptest"
	"net/url"
	"testing"

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
