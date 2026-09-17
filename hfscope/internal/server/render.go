// Package server: render.go provides the Go entrypoints used by handlers to
// invoke templ components. Keeping these in one file makes the package
// surface obvious.
package server

import (
	"context"
	"net/http"

	"github.com/KooshaPari/hfscope/internal/server/templ"
)

func renderHome(w http.ResponseWriter, r *http.Request, d PageData) {
	templ.Home(d).Render(r.Context(), w)
}

func renderSearch(w http.ResponseWriter, r *http.Request, d PageData) {
	templ.Search(d).Render(r.Context(), w)
}

func renderResults(w http.ResponseWriter, r *http.Request, d PageData) {
	templ.Results(d).Render(r.Context(), w)
}

func renderResultsPartial(w http.ResponseWriter, r *http.Request, d PageData) {
	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	templ.ResultsPartial(d).Render(r.Context(), w)
}

func renderModel(w http.ResponseWriter, r *http.Request, d PageData) {
	templ.ModelDetail(d).Render(r.Context(), w)
}

func renderAbout(w http.ResponseWriter, r *http.Request, d PageData) {
	templ.About(d).Render(r.Context(), w)
}

func renderError(w http.ResponseWriter, _ *http.Request, d ErrorData) {
	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	templ.ErrorPage(d.Title, d.Message).Render(context.Background(), w)
}