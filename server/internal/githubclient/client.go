package githubclient

import (
	"context"
	"errors"
	"fmt"
	"net/http"
	"net/url"
	"strings"
	"time"

	"github.com/google/go-github/v55/github"
	"golang.org/x/oauth2"
)

// Client wraps the GitHub API client with helper methods tailored for the CapyCoding UI.
type Client struct {
	api *github.Client
}

const defaultPageSize = 20

type clientConfig struct {
	token      string
	httpClient *http.Client
	baseURL    string
	uploadURL  string
}

// ClientOption configures how a Client is constructed.
type ClientOption func(*clientConfig)

// WithToken overrides the token discovered from the environment.
func WithToken(token string) ClientOption {
	return func(cfg *clientConfig) {
		cfg.token = token
	}
}

// WithHTTPClient injects a custom http.Client.
func WithHTTPClient(client *http.Client) ClientOption {
	return func(cfg *clientConfig) {
		cfg.httpClient = client
	}
}

// WithBaseURLs overrides the REST and upload base URLs. Useful for tests.
func WithBaseURLs(baseURL, uploadURL string) ClientOption {
	return func(cfg *clientConfig) {
		cfg.baseURL = baseURL
		cfg.uploadURL = uploadURL
	}
}

// NewClient builds a GitHub API client authenticated via personal access token.
func NewClient(ctx context.Context, opts ...ClientOption) (*Client, error) {
	cfg := clientConfig{}
	for _, opt := range opts {
		opt(&cfg)
	}

	if cfg.token == "" {
		token, err := loadToken()
		if err != nil {
			return nil, fmt.Errorf("load github token: %w", err)
		}
		cfg.token = token
	}

	ts := oauth2.StaticTokenSource(&oauth2.Token{AccessToken: cfg.token})

	var httpClient *http.Client
	if cfg.httpClient != nil {
		base := cfg.httpClient
		httpClient = &http.Client{
			Transport:     &oauth2.Transport{Source: ts, Base: base.Transport},
			CheckRedirect: base.CheckRedirect,
			Jar:           base.Jar,
			Timeout:       base.Timeout,
		}
	} else {
		httpClient = oauth2.NewClient(ctx, ts)
	}

	api := github.NewClient(httpClient)

	if cfg.baseURL != "" {
		if err := setBaseURL(api, cfg.baseURL); err != nil {
			return nil, err
		}
	}

	if cfg.uploadURL != "" {
		if err := setUploadURL(api, cfg.uploadURL); err != nil {
			return nil, err
		}
	}

	return &Client{api: api}, nil
}

// NewFromGitHubClient wraps an existing github.Client instance.
func NewFromGitHubClient(api *github.Client) *Client {
	return &Client{api: api}
}

// CloneWithToken returns the current client when no override token is provided.
// When an override token is supplied it creates a new client instance that
// reuses the existing base and upload URLs but authenticates with the
// provided token.
func (c *Client) CloneWithToken(ctx context.Context, token string) (*Client, error) {
	if token == "" {
		return c, nil
	}

	var baseURL, uploadURL string
	if c.api.BaseURL != nil {
		baseURL = c.api.BaseURL.String()
	}
	if c.api.UploadURL != nil {
		uploadURL = c.api.UploadURL.String()
	}

	return NewClient(ctx, WithToken(token), WithBaseURLs(baseURL, uploadURL))
}

func setBaseURL(api *github.Client, raw string) error {
	parsed, err := parseURLWithTrailingSlash(raw)
	if err != nil {
		return fmt.Errorf("set base url: %w", err)
	}
	api.BaseURL = parsed
	return nil
}

func setUploadURL(api *github.Client, raw string) error {
	parsed, err := parseURLWithTrailingSlash(raw)
	if err != nil {
		return fmt.Errorf("set upload url: %w", err)
	}
	api.UploadURL = parsed
	return nil
}

func parseURLWithTrailingSlash(raw string) (*url.URL, error) {
	if raw == "" {
		return nil, errors.New("empty url")
	}
	if !strings.HasSuffix(raw, "/") {
		raw += "/"
	}
	return url.Parse(raw)
}

// PRStatus represents the minimal information the UI needs for each pull request.
type PRStatus struct {
	Number    int        `json:"number"`
	Title     string     `json:"title"`
	State     string     `json:"state"`
	URL       string     `json:"url"`
	UpdatedAt time.Time  `json:"updatedAt"`
	Author    string     `json:"author"`
	Merged    bool       `json:"merged,omitempty"`
	MergedAt  *time.Time `json:"mergedAt,omitempty"`
}

// WorkflowRun represents a single workflow run summary.
type WorkflowRun struct {
	ID         int64     `json:"id"`
	Name       string    `json:"name"`
	Status     string    `json:"status"`
	Conclusion string    `json:"conclusion"`
	HTMLURL    string    `json:"htmlUrl"`
	CreatedAt  time.Time `json:"createdAt"`
	UpdatedAt  time.Time `json:"updatedAt"`
}

// CommitMetrics summarises commit activity for a repository.
type CommitMetrics struct {
	Total    int            `json:"total"`
	ByAuthor map[string]int `json:"byAuthor"`
	Since    *time.Time     `json:"since,omitempty"`
	Until    *time.Time     `json:"until,omitempty"`
}

// ErrInvalidRepository indicates that the repository reference is incomplete.
var ErrInvalidRepository = errors.New("owner and repository name are required")

// ErrInvalidUser indicates that a username is required.
var ErrInvalidUser = errors.New("username is required")

// Repository identifies a GitHub repository.
type Repository struct {
	Owner string
	Name  string
}

// Validate ensures the repository reference is complete.
func (r Repository) Validate() error {
	if r.Owner == "" || r.Name == "" {
		return ErrInvalidRepository
	}
	return nil
}

// PullRequestOptions controls how pull requests are fetched.
type PullRequestOptions struct {
	State   string
	PerPage int
}

func (o *PullRequestOptions) normalise() {
	if o.PerPage <= 0 || o.PerPage > 100 {
		o.PerPage = defaultPageSize
	}
}

// WorkflowOptions controls how workflow runs are fetched.
type WorkflowOptions struct {
	Branch  string
	PerPage int
}

func (o *WorkflowOptions) normalise() {
	if o.PerPage <= 0 || o.PerPage > 100 {
		o.PerPage = defaultPageSize
	}
}

// CommitOptions controls how commits are aggregated.
type CommitOptions struct {
	Since *time.Time
	Until *time.Time
}

// PullRequestStatuses fetches the latest pull requests matching the provided state.
func (c *Client) PullRequestStatuses(ctx context.Context, repo Repository, opts PullRequestOptions) ([]PRStatus, error) {
	if err := repo.Validate(); err != nil {
		return nil, err
	}

	opts.normalise()

	prOpts := &github.PullRequestListOptions{
		State: opts.State,
		ListOptions: github.ListOptions{
			PerPage: opts.PerPage,
		},
		Sort:      "updated",
		Direction: "desc",
	}

	prs, _, err := c.api.PullRequests.List(ctx, repo.Owner, repo.Name, prOpts)
	if err != nil {
		return nil, err
	}

	statuses := make([]PRStatus, 0, len(prs))
	for _, pr := range prs {
		status := PRStatus{
			Number:    pr.GetNumber(),
			Title:     pr.GetTitle(),
			State:     pr.GetState(),
			URL:       pr.GetHTMLURL(),
			UpdatedAt: pr.GetUpdatedAt().Time,
			Author:    pr.GetUser().GetLogin(),
			Merged:    pr.GetMerged(),
		}
		if pr.MergedAt != nil {
			status.MergedAt = &pr.MergedAt.Time
		}
		statuses = append(statuses, status)
	}

	return statuses, nil
}

// UserPullRequestStatuses fetches the latest pull requests authored by the provided user.
func (c *Client) UserPullRequestStatuses(ctx context.Context, username string, opts PullRequestOptions) ([]PRStatus, error) {
	username = strings.TrimSpace(username)
	if username == "" {
		return nil, ErrInvalidUser
	}

	opts.normalise()

	query := []string{"type:pr", fmt.Sprintf("author:%s", username)}
	switch strings.ToLower(strings.TrimSpace(opts.State)) {
	case "open":
		query = append(query, "is:open")
	case "closed":
		query = append(query, "is:closed")
	case "merged":
		query = append(query, "is:merged")
	}

	searchOpts := &github.SearchOptions{
		Sort:  "updated",
		Order: "desc",
		ListOptions: github.ListOptions{
			PerPage: opts.PerPage,
		},
	}

	// Use go-github's Search.Issues which is already authenticated and optimized
	results, _, err := c.api.Search.Issues(ctx, strings.Join(query, " "), searchOpts)
	if err != nil {
		return nil, err
	}

	// Check if we're filtering for merged PRs
	queryStr := strings.Join(query, " ")
	isMergedQuery := strings.Contains(queryStr, "is:merged")

	statuses := make([]PRStatus, 0, len(results.Issues))
	for _, issue := range results.Issues {
		status := PRStatus{
			Number:    issue.GetNumber(),
			Title:     issue.GetTitle(),
			State:     issue.GetState(),
			URL:       issue.GetHTMLURL(),
			UpdatedAt: issue.GetUpdatedAt().Time,
		}
		if issue.User != nil {
			status.Author = issue.User.GetLogin()
		}

		// If we filtered for merged PRs, mark them all as merged
		// The GitHub Search API with "is:merged" only returns merged PRs
		if isMergedQuery {
			status.Merged = true
			// Note: merged_at timestamp is available in the raw API response but not exposed by go-github
			// For now, we indicate merged status but don't have the exact timestamp unless we make additional API calls
		}

		statuses = append(statuses, status)
	}

	return statuses, nil
}

// WorkflowRuns fetches the most recent workflow runs for the provided repository and branch.
func (c *Client) WorkflowRuns(ctx context.Context, repo Repository, opts WorkflowOptions) ([]WorkflowRun, error) {
	if err := repo.Validate(); err != nil {
		return nil, err
	}

	opts.normalise()

	wfOpts := &github.ListWorkflowRunsOptions{
		Branch: opts.Branch,
		ListOptions: github.ListOptions{
			PerPage: opts.PerPage,
		},
	}

	runs, _, err := c.api.Actions.ListRepositoryWorkflowRuns(ctx, repo.Owner, repo.Name, wfOpts)
	if err != nil {
		return nil, err
	}

	results := make([]WorkflowRun, 0, len(runs.WorkflowRuns))
	for _, run := range runs.WorkflowRuns {
		results = append(results, WorkflowRun{
			ID:         run.GetID(),
			Name:       run.GetName(),
			Status:     run.GetStatus(),
			Conclusion: run.GetConclusion(),
			HTMLURL:    run.GetHTMLURL(),
			CreatedAt:  run.GetCreatedAt().Time,
			UpdatedAt:  run.GetUpdatedAt().Time,
		})
	}

	return results, nil
}

// UserWorkflowRuns fetches workflow runs across repositories owned by the provided user.
func (c *Client) UserWorkflowRuns(ctx context.Context, username string, opts WorkflowOptions) ([]WorkflowRun, error) {
	username = strings.TrimSpace(username)
	if username == "" {
		return nil, ErrInvalidUser
	}

	opts.normalise()
	remaining := opts.PerPage

	repoOpts := &github.RepositoryListOptions{
		Type:      "owner",
		Sort:      "pushed",
		Direction: "desc",
		ListOptions: github.ListOptions{
			PerPage: 50,
		},
	}

	var runs []WorkflowRun
	for {
		repos, resp, err := c.api.Repositories.List(ctx, username, repoOpts)
		if err != nil {
			return nil, err
		}

		for _, repo := range repos {
			if remaining <= 0 {
				return runs, nil
			}

			owner := ""
			if repo.Owner != nil {
				owner = repo.Owner.GetLogin()
			}
			if owner == "" {
				owner = username
			}

			repoRuns, err := c.WorkflowRuns(ctx, Repository{Owner: owner, Name: repo.GetName()}, WorkflowOptions{
				Branch:  opts.Branch,
				PerPage: remaining,
			})
			if err != nil {
				return nil, err
			}

			if len(repoRuns) > remaining {
				repoRuns = repoRuns[:remaining]
			}

			runs = append(runs, repoRuns...)
			remaining -= len(repoRuns)
		}

		if resp.NextPage == 0 || remaining <= 0 {
			break
		}
		repoOpts.Page = resp.NextPage
	}

	return runs, nil
}

// CommitCount returns commit metrics for a repository in a time window.
func (c *Client) CommitCount(ctx context.Context, repo Repository, opts CommitOptions) (*CommitMetrics, error) {
	if err := repo.Validate(); err != nil {
		return nil, err
	}

	listOpts := &github.CommitsListOptions{
		ListOptions: github.ListOptions{PerPage: 100},
	}

	if opts.Since != nil {
		listOpts.Since = *opts.Since
	}

	if opts.Until != nil {
		listOpts.Until = *opts.Until
	}

	metrics := &CommitMetrics{
		ByAuthor: make(map[string]int),
		Since:    opts.Since,
		Until:    opts.Until,
	}

	total := 0
	for {
		commits, resp, err := c.api.Repositories.ListCommits(ctx, repo.Owner, repo.Name, listOpts)
		if err != nil {
			return nil, err
		}

		for _, commit := range commits {
			author := "unknown"
			if commit.Author != nil {
				author = commit.Author.GetLogin()
			} else if commit.Commit != nil && commit.Commit.Author != nil {
				author = commit.Commit.Author.GetName()
			}
			metrics.ByAuthor[author]++
			total++
		}

		if resp.NextPage == 0 {
			break
		}
		listOpts.Page = resp.NextPage
	}

	metrics.Total = total
	return metrics, nil
}

// UserCommitCount aggregates commit metrics across all repositories owned by the provided user.
func (c *Client) UserCommitCount(ctx context.Context, username string, opts CommitOptions) (*CommitMetrics, error) {
	username = strings.TrimSpace(username)
	if username == "" {
		return nil, ErrInvalidUser
	}

	query := []string{fmt.Sprintf("author:%s", username)}
	// GitHub Search API requires range format (YYYY-MM-DD..YYYY-MM-DD) when both since and until are provided
	// Using separate >= and <= operators doesn't work correctly and causes the API to ignore the filters
	if opts.Since != nil && opts.Until != nil {
		query = append(query, fmt.Sprintf("committer-date:%s..%s",
			opts.Since.Format("2006-01-02"),
			opts.Until.Format("2006-01-02")))
	} else if opts.Since != nil {
		query = append(query, fmt.Sprintf("committer-date:>=%s", opts.Since.Format(time.RFC3339)))
	} else if opts.Until != nil {
		query = append(query, fmt.Sprintf("committer-date:<=%s", opts.Until.Format(time.RFC3339)))
	}

	searchOpts := &github.SearchOptions{
		Sort:  "committer-date",
		Order: "desc",
		ListOptions: github.ListOptions{
			PerPage: 100,
		},
	}

	metrics := &CommitMetrics{
		ByAuthor: make(map[string]int),
		Since:    opts.Since,
		Until:    opts.Until,
	}

	for {
		results, resp, err := c.api.Search.Commits(ctx, strings.Join(query, " "), searchOpts)
		if err != nil {
			return nil, err
		}

		for _, commit := range results.Commits {
			author := "unknown"
			if commit.Author != nil {
				author = commit.Author.GetLogin()
			} else if commit.Commit != nil && commit.Commit.Author != nil {
				author = commit.Commit.Author.GetName()
			}
			metrics.ByAuthor[author]++
			metrics.Total++
		}

		if resp.NextPage == 0 {
			break
		}
		searchOpts.Page = resp.NextPage
	}

	return metrics, nil
}
