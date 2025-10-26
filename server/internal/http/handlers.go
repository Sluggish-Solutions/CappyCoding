package http

import (
	"context"
	"errors"
	"net/http"
	"strconv"
	"strings"
	"time"

	"github.com/labstack/echo/v4"

	"cappycoding/server/internal/claude"
	"cappycoding/server/internal/githubclient"
)

var newGitHubClient = githubclient.NewClient

type claudeMetricsPayload struct {
	Timestamp           string  `json:"timestamp"`
	WindowHours         float64 `json:"window_hours"`
	BurnRatePerHour     float64 `json:"burn_rate_per_hour"`
	TotalCostUSD        float64 `json:"total_cost_usd"`
	InputTokens         int64   `json:"input_tokens"`
	OutputTokens        int64   `json:"output_tokens"`
	CacheCreationTokens int64   `json:"cache_creation_tokens"`
	CacheReadTokens     int64   `json:"cache_read_tokens"`
	TotalTokens         int64   `json:"total_tokens"`
	SessionCount        int     `json:"session_count"`
	ActiveSessionID     string  `json:"active_session_id"`
	LastActivity        string  `json:"last_activity"`
	Source              string  `json:"source"`
}

// RegisterRoutes wires the metrics endpoints on the provided Echo instance.
func RegisterRoutes(e *echo.Echo, client *githubclient.Client, claudeStore *claude.Store) {
	e.GET("/metrics/prs", func(c echo.Context) error {
		resolvedClient, err := resolveClient(c.Request().Context(), client, extractGitHubToken(c))
		if err != nil {
			return respondError(c, err)
		}

		user := c.QueryParam("user")
		opts := githubclient.PullRequestOptions{
			State:   c.QueryParam("state"),
			PerPage: queryParamInt(c, "per_page", 20),
		}

		prs, err := resolvedClient.UserPullRequestStatuses(c.Request().Context(), user, opts)
		if err != nil {
			return respondError(c, err)
		}

		return c.JSON(http.StatusOK, prs)
	})

	e.GET("/metrics/workflows", func(c echo.Context) error {
		resolvedClient, err := resolveClient(c.Request().Context(), client, extractGitHubToken(c))
		if err != nil {
			return respondError(c, err)
		}

		user := c.QueryParam("user")
		opts := githubclient.WorkflowOptions{
			Branch:  c.QueryParam("branch"),
			PerPage: queryParamInt(c, "per_page", 20),
		}

		runs, err := resolvedClient.UserWorkflowRuns(c.Request().Context(), user, opts)
		if err != nil {
			return respondError(c, err)
		}

		return c.JSON(http.StatusOK, runs)
	})

	e.GET("/metrics/commits", func(c echo.Context) error {
		resolvedClient, err := resolveClient(c.Request().Context(), client, extractGitHubToken(c))
		if err != nil {
			return respondError(c, err)
		}

		user := c.QueryParam("user")
		since, err := parseTime(c.QueryParam("since"))
		if err != nil {
			return c.JSON(http.StatusBadRequest, map[string]string{"error": err.Error()})
		}
		until, err := parseTime(c.QueryParam("until"))
		if err != nil {
			return c.JSON(http.StatusBadRequest, map[string]string{"error": err.Error()})
		}

		metrics, err := resolvedClient.UserCommitCount(c.Request().Context(), user, githubclient.CommitOptions{Since: since, Until: until})
		if err != nil {
			return respondError(c, err)
		}

		return c.JSON(http.StatusOK, metrics)
	})

	if claudeStore != nil {
		e.GET("/metrics/claude", func(c echo.Context) error {
			latest, ok := claudeStore.Latest()
			if !ok {
				return c.JSON(http.StatusNotFound, map[string]string{"error": "no claude metrics available"})
			}
			return c.JSON(http.StatusOK, latest)
		})

		e.GET("/metrics/claude/history", func(c echo.Context) error {
			return c.JSON(http.StatusOK, claudeStore.History())
		})

		e.POST("/metrics/claude", func(c echo.Context) error {
			var payload claudeMetricsPayload
			if err := c.Bind(&payload); err != nil {
				return c.JSON(http.StatusBadRequest, map[string]string{"error": "invalid payload"})
			}

			snapshot, err := convertClaudePayload(payload)
			if err != nil {
				return c.JSON(http.StatusBadRequest, map[string]string{"error": err.Error()})
			}

			if err := claudeStore.Update(snapshot); err != nil {
				return c.JSON(http.StatusBadRequest, map[string]string{"error": err.Error()})
			}

			return c.JSON(http.StatusOK, snapshot)
		})
	}
}

func queryParamInt(c echo.Context, key string, fallback int) int {
	if value := c.QueryParam(key); value != "" {
		if parsed, err := strconv.Atoi(value); err == nil {
			return parsed
		}
	}

	return fallback
}

func parseTime(raw string) (*time.Time, error) {
	if raw == "" {
		return nil, nil
	}
	t, err := time.Parse(time.RFC3339, raw)
	if err != nil {
		return nil, err
	}
	return &t, nil
}

func respondError(c echo.Context, err error) error {
	status := http.StatusInternalServerError
	if errors.Is(err, githubclient.ErrInvalidRepository) {
		status = http.StatusBadRequest
	} else if errors.Is(err, githubclient.ErrInvalidUser) {
		status = http.StatusBadRequest
	} else if errors.Is(err, githubclient.ErrMissingToken) {
		status = http.StatusUnauthorized
	}
	return c.JSON(status, map[string]string{"error": err.Error()})
}

func extractGitHubToken(c echo.Context) string {
	auth := c.Request().Header.Get("Authorization")
	if auth != "" {
		lower := strings.ToLower(auth)
		switch {
		case strings.HasPrefix(lower, "token "):
			return strings.TrimSpace(auth[6:])
		case strings.HasPrefix(lower, "bearer "):
			return strings.TrimSpace(auth[7:])
		}
	}

	if token := c.Request().Header.Get("X-GitHub-Token"); token != "" {
		return strings.TrimSpace(token)
	}

	return ""
}

func resolveClient(ctx context.Context, base *githubclient.Client, token string) (*githubclient.Client, error) {
	if base == nil {
		if token == "" {
			return nil, githubclient.ErrMissingToken
		}
		return newGitHubClient(ctx, githubclient.WithToken(token))
	}

	return base.CloneWithToken(ctx, token)
}

func convertClaudePayload(payload claudeMetricsPayload) (claude.Metrics, error) {
	timestamp, err := time.Parse(time.RFC3339, payload.Timestamp)
	if err != nil {
		return claude.Metrics{}, err
	}

	lastActivity, err := time.Parse(time.RFC3339, payload.LastActivity)
	if err != nil {
		return claude.Metrics{}, err
	}

	snapshot := claude.Metrics{
		Timestamp:           timestamp,
		WindowHours:         payload.WindowHours,
		BurnRatePerHour:     payload.BurnRatePerHour,
		TotalCostUSD:        payload.TotalCostUSD,
		InputTokens:         payload.InputTokens,
		OutputTokens:        payload.OutputTokens,
		CacheCreationTokens: payload.CacheCreationTokens,
		CacheReadTokens:     payload.CacheReadTokens,
		TotalTokens:         payload.TotalTokens,
		SessionCount:        payload.SessionCount,
		ActiveSessionID:     strings.TrimSpace(payload.ActiveSessionID),
		LastActivity:        lastActivity,
		Source:              strings.TrimSpace(payload.Source),
	}

	if err := snapshot.Validate(); err != nil {
		return claude.Metrics{}, err
	}

	return snapshot, nil
}
