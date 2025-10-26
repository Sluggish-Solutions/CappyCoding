package claude

import (
	"testing"
	"time"
)

func TestStoreUpdateAndHistory(t *testing.T) {
	t.Parallel()

	store := NewStore(2)

	now := time.Now().UTC()
	metrics := Metrics{
		Timestamp:           now,
		WindowHours:         1,
		BurnRatePerHour:     1.23,
		TotalCostUSD:        4.56,
		InputTokens:         10,
		OutputTokens:        20,
		CacheCreationTokens: 1,
		CacheReadTokens:     2,
		TotalTokens:         33,
		SessionCount:        2,
		LastActivity:        now,
	}

	if err := store.Update(metrics); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	latest, ok := store.Latest()
	if !ok {
		t.Fatalf("expected metrics to be available")
	}
	if latest.TotalCostUSD != metrics.TotalCostUSD {
		t.Fatalf("unexpected latest metrics: %+v", latest)
	}

	second := metrics
	second.Timestamp = now.Add(1 * time.Hour)
	second.LastActivity = second.Timestamp
	second.TotalCostUSD = 7.89

	if err := store.Update(second); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	history := store.History()
	if len(history) != 2 {
		t.Fatalf("expected 2 history entries, got %d", len(history))
	}

	third := metrics
	third.Timestamp = now.Add(2 * time.Hour)
	third.LastActivity = third.Timestamp
	third.TotalCostUSD = 9.99

	if err := store.Update(third); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	history = store.History()
	if len(history) != 2 {
		t.Fatalf("history should be capped at 2 entries, got %d", len(history))
	}
	if history[0].TotalCostUSD != second.TotalCostUSD {
		t.Fatalf("expected oldest entry to be second update")
	}
}

func TestMetricsValidation(t *testing.T) {
	t.Parallel()

	invalid := Metrics{}
	if err := invalid.Validate(); err == nil {
		t.Fatalf("expected error for invalid metrics")
	}

	now := time.Now().UTC()
	valid := Metrics{
		Timestamp:           now,
		WindowHours:         2,
		BurnRatePerHour:     0,
		TotalCostUSD:        0,
		TotalTokens:         0,
		InputTokens:         0,
		OutputTokens:        0,
		CacheCreationTokens: 0,
		CacheReadTokens:     0,
		SessionCount:        0,
		LastActivity:        now,
	}

	if err := valid.Validate(); err != nil {
		t.Fatalf("unexpected error for valid metrics: %v", err)
	}
}
