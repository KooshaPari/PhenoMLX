// Package cache provides a small TTL cache for HTTP responses keyed by URL.
// It is used to dedupe HuggingFace API calls and absorb burst traffic.
package cache

import (
	"sync"
	"time"
)

// Entry is one cached response.
type Entry struct {
	Body       []byte
	Header     map[string]string
	StatusCode int
	StoredAt   time.Time
}

// TTL is a thread-safe map of string keys to cache entries, each evicted
// after the configured TTL.
type TTL struct {
	ttl  time.Duration
	mu   sync.RWMutex
	data map[string]Entry
}

// New returns a TTL cache that retains entries for d.
func New(d time.Duration) *TTL {
	return &TTL{
		ttl:  d,
		data: make(map[string]Entry),
	}
}

// Get returns a cached entry and true when present and fresh; otherwise false.
func (c *TTL) Get(key string) (Entry, bool) {
	c.mu.RLock()
	defer c.mu.RUnlock()
	e, ok := c.data[key]
	if !ok {
		return Entry{}, false
	}
	if time.Since(e.StoredAt) > c.ttl {
		return Entry{}, false
	}
	return e, true
}

// Set stores an entry. Existing entries with the same key are overwritten.
func (c *TTL) Set(key string, body []byte, headers map[string]string, status int) {
	c.mu.Lock()
	defer c.mu.Unlock()
	c.data[key] = Entry{
		Body:       body,
		Header:     headers,
		StatusCode: status,
		StoredAt:   time.Now(),
	}
}

// Len reports how many live entries the cache holds.
func (c *TTL) Len() int {
	c.mu.RLock()
	defer c.mu.RUnlock()
	return len(c.data)
}

// Janitor periodically evicts expired entries. Stop the returned channel to
// shut it down.
func (c *TTL) Janitor(interval time.Duration) (stop func()) {
	t := time.NewTicker(interval)
	stopCh := make(chan struct{})
	go func() {
		for {
			select {
			case <-t.C:
				c.evict()
			case <-stopCh:
				t.Stop()
				return
			}
		}
	}()
	return func() { close(stopCh) }
}

func (c *TTL) evict() {
	now := time.Now()
	c.mu.Lock()
	for k, e := range c.data {
		if now.Sub(e.StoredAt) > c.ttl {
			delete(c.data, k)
		}
	}
	c.mu.Unlock()
}