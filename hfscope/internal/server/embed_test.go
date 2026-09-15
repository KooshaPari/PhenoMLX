package server

import (
	"encoding/json"
	"io"
	"log/slog"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"github.com/kooshapari/hfscope/internal/config"
	"github.com/kooshapari/hfscope/internal/hfapi"
)

// newEmbedTestServer builds a Server whose upstream is a stub HuggingFace API
// serving one model list page (with a rel="next" Link header) and one model
// detail document. It returns the server plus a pointer to a slice recording
// the Authorization header of every upstream request.
func newEmbedTestServer(t *testing.T) (*Server, *[]string) {
	t.Helper()
	var authLog []string

	upstream := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		authLog = append(authLog, r.Header.Get("Authorization"))
		w.Header().Set("Content-Type", "application/json")
		switch {
		case r.URL.Path == "/api/models":
			w.Header().Set("Link", `</api/models?cursor=abc123>; rel="next"`)
			_ = json.NewEncoder(w).Encode([]hfapi.Model{{ID: "org/demo", Downloads: 10, Likes: 2}})
		case strings.HasPrefix(r.URL.Path, "/api/models/"):
			_ = json.NewEncoder(w).Encode(map[string]any{
				"id": strings.TrimPrefix(r.URL.Path, "/api/models/"),
			})
		default:
			http.NotFound(w, r)
		}
	}))
	t.Cleanup(upstream.Close)

	cfg := config.Config{HFBase: upstream.URL}
	hf := hfapi.New(hfapi.Options{BaseURL: upstream.URL})
	log := slog.New(slog.NewTextHandler(io.Discard, nil))
	return New(cfg, hf, log), &authLog
}

// TestEmbedResultsServesFragmentWithCursorHeader pins the /embed/results
// contract consumed by the browser extension and third-party embeds: 200,
// open CORS, the next-page cursor surfaced via X-HFScope-Next-Cursor, and a
// chrome-less fragment that still renders result cards.
func TestEmbedResultsServesFragmentWithCursorHeader(t *testing.T) {
	srv, _ := newEmbedTestServer(t)

	req := httptest.NewRequest(http.MethodGet, "/embed/results?q=demo&limit=5", nil)
	rec := httptest.NewRecorder()
	srv.Handler().ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("status = %d, want 200; body: %s", rec.Code, rec.Body.String())
	}
	if got := rec.Header().Get("Access-Control-Allow-Origin"); got != "*" {
		t.Errorf("Access-Control-Allow-Origin = %q, want *", got)
	}
	if got := rec.Header().Get("X-HFScope-Next-Cursor"); got != "abc123" {
		t.Errorf("X-HFScope-Next-Cursor = %q, want abc123", got)
	}
	if body := rec.Body.String(); !strings.Contains(body, "org/demo") {
		t.Errorf("fragment does not render result cards; body: %s", body)
	}
}

// TestEmbedModelServesDetailFragment pins the /embed/model/{id} contract:
// the single-model fragment renders without layout chrome.
func TestEmbedModelServesDetailFragment(t *testing.T) {
	srv, _ := newEmbedTestServer(t)

	req := httptest.NewRequest(http.MethodGet, "/embed/model/gpt2", nil)
	rec := httptest.NewRecorder()
	srv.Handler().ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("status = %d, want 200; body: %s", rec.Code, rec.Body.String())
	}
	if got := rec.Header().Get("Access-Control-Allow-Origin"); got != "*" {
		t.Errorf("Access-Control-Allow-Origin = %q, want *", got)
	}
	if body := rec.Body.String(); !strings.Contains(body, "gpt2") {
		t.Errorf("fragment does not render model detail; body: %s", body)
	}
}

// TestAPIModelsReturnsCursorJSON pins the JSON API contract: the cursor for
// the next page is exposed both as a header and inside the JSON body.
func TestAPIModelsReturnsCursorJSON(t *testing.T) {
	srv, _ := newEmbedTestServer(t)

	req := httptest.NewRequest(http.MethodGet, "/api/models?limit=5", nil)
	rec := httptest.NewRecorder()
	srv.Handler().ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("status = %d, want 200; body: %s", rec.Code, rec.Body.String())
	}
	if got := rec.Header().Get("X-HFScope-Next-Cursor"); got != "abc123" {
		t.Errorf("X-HFScope-Next-Cursor = %q, want abc123", got)
	}
	var payload struct {
		Type   string `json:"type"`
		Cursor string `json:"cursor"`
		Models []struct {
			ID string `json:"id"`
		} `json:"models"`
	}
	if err := json.Unmarshal(rec.Body.Bytes(), &payload); err != nil {
		t.Fatalf("decode body: %v; body: %s", err, rec.Body.String())
	}
	if payload.Cursor != "abc123" {
		t.Errorf("json cursor = %q, want abc123", payload.Cursor)
	}
	if len(payload.Models) != 1 || payload.Models[0].ID != "org/demo" {
		t.Errorf("json models = %+v, want one org/demo", payload.Models)
	}
}

// TestEmbedCallerTokenOverridesUpstreamAuth pins the per-request token path:
// a caller-supplied Authorization header must reach HuggingFace as a Bearer
// token via the client copy, not leak into other requests.
func TestEmbedCallerTokenOverridesUpstreamAuth(t *testing.T) {
	srv, authLog := newEmbedTestServer(t)

	req := httptest.NewRequest(http.MethodGet, "/embed/results?q=demo", nil)
	req.Header.Set("Authorization", "Bearer tok-xyz")
	rec := httptest.NewRecorder()
	srv.Handler().ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("status = %d, want 200; body: %s", rec.Code, rec.Body.String())
	}
	if len(*authLog) != 1 {
		t.Fatalf("upstream saw %d requests, want 1", len(*authLog))
	}
	if got := (*authLog)[0]; got != "Bearer tok-xyz" {
		t.Errorf("upstream Authorization = %q, want %q", got, "Bearer tok-xyz")
	}
}
