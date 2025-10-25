package main

import (
	"context"
	"errors"
	"log"
	"net/http"
	"os"
	"os/signal"
	"syscall"
	"time"

	echov4 "github.com/labstack/echo/v4"
	"github.com/labstack/echo/v4/middleware"

	"cappycoding/server/internal/githubclient"
	httpHandlers "cappycoding/server/internal/http"
)

func main() {
	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer stop()

	client, err := githubclient.NewClient(ctx)
	if err != nil {
		if errors.Is(err, githubclient.ErrMissingToken) {
			log.Printf("github token not configured: %v", err)
			log.Printf("requests must include Authorization or X-GitHub-Token headers; continuing without default credentials")
			client = nil
		} else {
			log.Fatalf("failed to create github client: %v", err)
		}
	}

	e := echov4.New()
	e.HideBanner = true
	e.Use(middleware.Logger())
	e.Use(middleware.Recover())

	httpHandlers.RegisterRoutes(e, client)

	srv := &http.Server{
		Addr:         addr(),
		Handler:      e,
		ReadTimeout:  15 * time.Second,
		WriteTimeout: 15 * time.Second,
	}

	errCh := make(chan error, 1)
	go func() {
		log.Printf("Starting metrics server on %s", srv.Addr)
		errCh <- srv.ListenAndServe()
	}()

	select {
	case <-ctx.Done():
		shutdownCtx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
		defer cancel()
		if err := srv.Shutdown(shutdownCtx); err != nil {
			log.Printf("graceful shutdown failed: %v", err)
		}
	case err := <-errCh:
		if err != nil && !errors.Is(err, http.ErrServerClosed) {
			log.Fatalf("server error: %v", err)
		}
	}
}

func addr() string {
	if value := os.Getenv("CAPYCODING_SERVER_ADDR"); value != "" {
		return value
	}
	return ":8080"
}
