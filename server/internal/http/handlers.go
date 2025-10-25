package http

import (
	"errors"
	"net/http"
	"strconv"
	"time"

	"github.com/labstack/echo/v4"

	"cappycoding/server/internal/githubclient"
)

// RegisterRoutes wires the metrics endpoints on the provided Echo instance.
func RegisterRoutes(e *echo.Echo, client *githubclient.Client) {
	e.GET("/metrics/prs", func(c echo.Context) error {
		repo := githubclient.Repository{Owner: c.QueryParam("owner"), Name: c.QueryParam("repo")}
		opts := githubclient.PullRequestOptions{
			State:   c.QueryParam("state"),
			PerPage: queryParamInt(c, "per_page", 20),
		}

		prs, err := client.PullRequestStatuses(c.Request().Context(), repo, opts)
		if err != nil {
			return respondError(c, err)
		}

		return c.JSON(http.StatusOK, prs)
	})

	e.GET("/metrics/workflows", func(c echo.Context) error {
		repo := githubclient.Repository{Owner: c.QueryParam("owner"), Name: c.QueryParam("repo")}
		opts := githubclient.WorkflowOptions{
			Branch:  c.QueryParam("branch"),
			PerPage: queryParamInt(c, "per_page", 20),
		}

		runs, err := client.WorkflowRuns(c.Request().Context(), repo, opts)
		if err != nil {
			return respondError(c, err)
		}

		return c.JSON(http.StatusOK, runs)
	})

	e.GET("/metrics/commits", func(c echo.Context) error {
		repo := githubclient.Repository{Owner: c.QueryParam("owner"), Name: c.QueryParam("repo")}
		since, err := parseTime(c.QueryParam("since"))
		if err != nil {
			return c.JSON(http.StatusBadRequest, map[string]string{"error": err.Error()})
		}
		until, err := parseTime(c.QueryParam("until"))
		if err != nil {
			return c.JSON(http.StatusBadRequest, map[string]string{"error": err.Error()})
		}

		metrics, err := client.CommitCount(c.Request().Context(), repo, githubclient.CommitOptions{Since: since, Until: until})
		if err != nil {
			return respondError(c, err)
		}

		return c.JSON(http.StatusOK, metrics)
	})
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
	}
	return c.JSON(status, map[string]string{"error": err.Error()})
}
