package claude

import (
	"errors"
	"sync"
	"time"
)

// Metrics represents a snapshot of Claude usage metrics aggregated over a
// specific window of time.
type Metrics struct {
	Timestamp           time.Time `json:"timestamp"`
	WindowHours         float64   `json:"window_hours"`
	BurnRatePerHour     float64   `json:"burn_rate_per_hour"`
	TotalCostUSD        float64   `json:"total_cost_usd"`
	InputTokens         int64     `json:"input_tokens"`
	OutputTokens        int64     `json:"output_tokens"`
	CacheCreationTokens int64     `json:"cache_creation_tokens"`
	CacheReadTokens     int64     `json:"cache_read_tokens"`
	TotalTokens         int64     `json:"total_tokens"`
	SessionCount        int       `json:"session_count"`
	ActiveSessionID     string    `json:"active_session_id,omitempty"`
	LastActivity        time.Time `json:"last_activity"`
	Source              string    `json:"source,omitempty"`
}

// Validate ensures the metrics snapshot contains sane data.
func (m Metrics) Validate() error {
	if m.WindowHours <= 0 {
		return errors.New("window hours must be greater than zero")
	}
	if m.Timestamp.IsZero() {
		return errors.New("timestamp is required")
	}
	if m.LastActivity.IsZero() {
		return errors.New("last activity timestamp is required")
	}
	if m.TotalTokens < 0 || m.InputTokens < 0 || m.OutputTokens < 0 || m.CacheCreationTokens < 0 || m.CacheReadTokens < 0 {
		return errors.New("token counts must be non-negative")
	}
	if m.TotalCostUSD < 0 {
		return errors.New("total cost must be non-negative")
	}
	if m.BurnRatePerHour < 0 {
		return errors.New("burn rate must be non-negative")
	}
	return nil
}

// Store keeps track of the latest Claude metrics and a bounded history so the
// ESP32 can request both current and historical data.
type Store struct {
	mu         sync.RWMutex
	latest     *Metrics
	history    []Metrics
	maxHistory int
}

// NewStore builds a Store that keeps the provided number of historical entries
// in-memory.
func NewStore(maxHistory int) *Store {
	if maxHistory < 1 {
		maxHistory = 1
	}
	return &Store{maxHistory: maxHistory}
}

// Update replaces the latest snapshot and appends it to the history buffer.
func (s *Store) Update(m Metrics) error {
	if err := m.Validate(); err != nil {
		return err
	}

	s.mu.Lock()
	defer s.mu.Unlock()

	s.latest = &m
	s.history = append(s.history, m)
	if len(s.history) > s.maxHistory {
		overflow := len(s.history) - s.maxHistory
		s.history = append([]Metrics(nil), s.history[overflow:]...)
	}
	return nil
}

// Latest returns the most recently stored snapshot. The boolean return value is
// false when no snapshot has been recorded yet.
func (s *Store) Latest() (Metrics, bool) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	if s.latest == nil {
		return Metrics{}, false
	}
	return *s.latest, true
}

// History returns a copy of the stored history, ordered from oldest to newest.
func (s *Store) History() []Metrics {
	s.mu.RLock()
	defer s.mu.RUnlock()

	out := make([]Metrics, len(s.history))
	copy(out, s.history)
	return out
}
