// Package server: embed.go serves the federated embed surface for browser
// extensions, Tauri/Flutter/RN webviews, iframes, and web components.
// Endpoints return just the rendered card grid — no sidebar, no chrome.
// CORS is open by default for cross-origin embedding.
package server

import (
	"encoding/json"
	"net/http"
	"strconv"
	"strings"

	"github.com/kooshapari/hfscope/internal/hfapi"
	"github.com/kooshapari/hfscope/internal/views"
)

// corsHeaders sets permissive CORS on every embed response.
func corsHeaders(w http.ResponseWriter) {
	w.Header().Set("Access-Control-Allow-Origin", "*")
	w.Header().Set("Access-Control-Allow-Methods", "GET, OPTIONS")
	w.Header().Set("Access-Control-Allow-Headers", "Authorization, Content-Type, X-HFScope-Token")
	w.Header().Set("Access-Control-Expose-Headers", "X-HFScope-Total, X-HFScope-Next-Cursor")
	w.Header().Set("Access-Control-Max-Age", "86400")
	w.Header().Set("Vary", "Origin")
}

// withCORS wraps a handler with CORS headers + OPTIONS preflight.
func (s *Server) withCORS(h http.HandlerFunc) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		corsHeaders(w)
		if r.Method == http.MethodOptions {
			w.WriteHeader(http.StatusNoContent)
			return
		}
		h(w, r)
	}
}

// embedToken extracts the caller's HF token from the Authorization header,
// X-HFScope-Token header, or ?token= query param.
func embedToken(r *http.Request) string {
	auth := r.Header.Get("Authorization")
	if strings.HasPrefix(strings.ToLower(auth), "bearer ") {
		return strings.TrimSpace(auth[len("Bearer "):])
	}
	if v := r.Header.Get("X-HFScope-Token"); v != "" {
		return strings.TrimSpace(v)
	}
	if v := r.URL.Query().Get("token"); v != "" {
		return strings.TrimSpace(v)
	}
	return ""
}

// listByType routes between models/spaces/datasets list endpoints.
func listByType(r *http.Request, hf *hfapi.Client, params hfapi.SearchParams) (hfapi.ListResult, error) {
	ctx := r.Context()
	switch params.Kind {
	case "spaces":
		return hf.ListSpaces(ctx, params)
	case "datasets":
		return hf.ListDatasets(ctx, params)
	default:
		params.Kind = "models"
		return hf.ListModels(ctx, params)
	}
}

// handleEmbedResults serves /embed/results — a minimal HTML fragment with
// just the card grid + toolbar, no chrome. Consumed by the Chrome extension
// side panel, Tauri webview, iframes, and web components.
func (s *Server) handleEmbedResults(w http.ResponseWriter, r *http.Request) {
	params := parseSearch(r)
	if params.Kind == "" {
		params.Kind = "models"
	}
	if l, _ := strconv.Atoi(r.URL.Query().Get("limit")); l > 0 && l <= 100 {
		params.Limit = l
	}
	if c := r.URL.Query().Get("cursor"); c != "" {
		params.Cursor = c
	}

	hf := s.hf
	// If the caller sent a token, use it for upstream calls.
	if tok := embedToken(r); tok != "" {
		hf = s.hf.WithToken(tok)
	}

	res, err := listByType(r, hf, params)
	if err != nil {
		corsHeaders(w)
		http.Error(w, "upstream error: "+err.Error(), http.StatusBadGateway)
		return
	}

	data := views.PageData{
		Params:    params,
		Result:    res,
		HFBase:    s.cfg.HFBase,
		Path:      r.URL.Path,
		IsProd:    s.cfg.IsProd(),
		ActiveNav: "results",
	}

	corsHeaders(w)
	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	w.Header().Set("X-HFScope-Total", strconv.Itoa(res.Total))
	if res.NextLink != "" {
		w.Header().Set("X-HFScope-Next-Cursor", res.NextLink)
	}
	w.Header().Set("Cache-Control", "public, max-age=30, stale-while-revalidate=120")

	_ = renderEmbedResults(w, r, data)
}

// handleEmbedModel serves /embed/model/{id} — a single model as an HTML fragment.
func (s *Server) handleEmbedModel(w http.ResponseWriter, r *http.Request) {
	id := strings.TrimPrefix(r.URL.Path, "/embed/model/")
	id = strings.TrimSuffix(id, "/")
	if id == "" || strings.Contains(id, "..") {
		corsHeaders(w)
		http.Error(w, "missing or invalid model id", http.StatusBadRequest)
		return
	}

	hf := s.hf
	if tok := embedToken(r); tok != "" {
		hf = s.hf.WithToken(tok)
	}

	d, err := hf.GetModelDetail(r.Context(), id)
	if err != nil {
		corsHeaders(w)
		http.Error(w, "upstream error: "+err.Error(), http.StatusBadGateway)
		return
	}

	data := views.PageData{
		ModelDetail: d,
		Summary:     hfapi.Summarize(d.Model),
		HFBase:      s.cfg.HFBase,
		Path:        r.URL.Path,
	}

	corsHeaders(w)
	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	w.Header().Set("Cache-Control", "public, max-age=120, stale-while-revalidate=600")

	_ = renderEmbedModel(w, r, data)
}

// handleAPIModels serves /api/models, /api/spaces, /api/datasets as JSON.
func (s *Server) handleAPIModels(w http.ResponseWriter, r *http.Request) {
	params := parseSearch(r)
	if l, _ := strconv.Atoi(r.URL.Query().Get("limit")); l > 0 && l <= 100 {
		params.Limit = l
	}
	if c := r.URL.Query().Get("cursor"); c != "" {
		params.Cursor = c
	}

	hf := s.hf
	if tok := embedToken(r); tok != "" {
		hf = s.hf.WithToken(tok)
	}

	res, err := listByType(r, hf, params)
	if err != nil {
		corsHeaders(w)
		http.Error(w, `{"error":"`+err.Error()+`"}`, http.StatusBadGateway)
		return
	}

	corsHeaders(w)
	w.Header().Set("Content-Type", "application/json; charset=utf-8")
	w.Header().Set("X-HFScope-Total", strconv.Itoa(res.Total))
	if res.NextLink != "" {
		w.Header().Set("X-HFScope-Next-Cursor", res.NextLink)
	}

	_ = json.NewEncoder(w).Encode(map[string]any{
		"type":   params.Kind,
		"total":  res.Total,
		"cursor": res.NextLink,
		"models": res.Models,
	})
}
