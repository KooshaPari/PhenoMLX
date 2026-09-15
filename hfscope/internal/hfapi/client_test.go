package hfapi

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"
)

// TestWithTokenSendsPerRequestAuth is the regression test for the embed
// surface contract: the server must be able to issue per-request tokened
// clients (from caller-supplied Authorization headers) without mutating the
// shared base client, and the token must reach upstream as a Bearer header.
func TestWithTokenSendsPerRequestAuth(t *testing.T) {
	var gotAuth []string
	upstream := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		gotAuth = append(gotAuth, r.Header.Get("Authorization"))
		w.Header().Set("Content-Type", "application/json")
		_ = json.NewEncoder(w).Encode([]Model{{ID: "org/demo"}})
	}))
	defer upstream.Close()

	base := New(Options{BaseURL: upstream.URL})
	if _, err := base.ListModels(context.Background(), SearchParams{}); err != nil {
		t.Fatalf("base ListModels: %v", err)
	}
	if _, err := base.WithToken("tok-abc").ListModels(context.Background(), SearchParams{}); err != nil {
		t.Fatalf("tokened ListModels: %v", err)
	}

	if len(gotAuth) != 2 {
		t.Fatalf("upstream saw %d requests, want 2", len(gotAuth))
	}
	if gotAuth[0] != "" {
		t.Errorf("base client sent Authorization %q, want none", gotAuth[0])
	}
	if gotAuth[1] != "Bearer tok-abc" {
		t.Errorf("WithToken client sent Authorization %q, want %q", gotAuth[1], "Bearer tok-abc")
	}
}
