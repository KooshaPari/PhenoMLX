// HFScope client-side filters. Powers the drag-range sliders for parameter
// size, creation date, and the numeric minimums for downloads / likes.
//
// The page renders every card with data-* attributes (data-params,
// data-downloads, data-likes, data-created). The sliders and number inputs
// filter the visible set in-place by toggling the .filtered-out class on
// each <article class="card">. No server round-trip is needed — the
// upstream result set is the entire pool we filter against.
//
// On form submit (Enter in a numeric input, click Apply) the active filter
// values are serialized into the URL so a deep link reproduces the same
// visible state. On reload we hydrate the controls from the URL.
//
// Behaviour:
//   - Sliders are dual-handle (a min and a max pointer on one track).
//   - Hidden inputs (params_min, params_max, created_after, created_before,
//     min_downloads, min_likes) are what actually gets submitted.
//   - "Any" is signalled by an empty hidden input value; the slider thumbs
//     sit at the endpoints in that case.
//   - URL hydration happens on DOMContentLoaded; if the URL has ?params_min=…
//     etc, the controls initialise to those values and filter immediately.
//   - When JS isn't available, the inputs are still submitted as a normal
//     GET form (the server doesn't apply them — we display them as
//     client-side hints via a small banner). The page remains usable.
(function () {
    'use strict';

    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', init);
    } else {
        init();
    }

    function init() {
        // Build all dual-handle sliders first; they read the URL for initial state.
        buildDualRange('#params-range', {
            minInput: '#params_min',
            maxInput: '#params_max',
            unit: 'params',
            onChange: applyFilters,
        });
        buildDualRange('#created-range', {
            minInput: '#created_after',
            maxInput: '#created_before',
            unit: 'date',
            onChange: applyFilters,
        });

        // Numeric minimums — debounced so we don't filter on every keystroke.
        var numInputs = document.querySelectorAll('[data-filter-input]');
        numInputs.forEach(function (el) {
            el.addEventListener('input', debounce(applyFilters, 150));
            el.addEventListener('change', applyFilters);
        });

        // Reset button.
        var reset = document.querySelector('[data-filter-reset]');
        if (reset) {
            reset.addEventListener('click', function (e) {
                e.preventDefault();
                clearAllControls();
                applyFilters();
            });
        }

        // Initial filter pass on hydration.
        applyFilters();
    }

    // ------- Dual-handle range slider ---------------------------------------
    // params: {
    //   trackSelector: '#params-range',
    //   minInput: '#params_min',  maxInput: '#params_max',
    //   unit: 'params' | 'date',
    //   onChange: fn
    // }
    function buildDualRange(trackSelector, opts) {
        var track = document.querySelector(trackSelector);
        if (!track) return;
        var minInput = document.querySelector(opts.minInput);
        var maxInput = document.querySelector(opts.maxInput);

        var minThumb = track.querySelector('.range-thumb.range-min');
        var maxThumb = track.querySelector('.range-thumb.range-max');
        var fill = track.querySelector('.range-fill');
        var minLabel = track.querySelector('.range-label-min');
        var maxLabel = track.querySelector('.range-label-max');

        // Bounds come from data-* on the track.
        var min = parseFloat(track.dataset.min);
        var max = parseFloat(track.dataset.max);
        var step = parseFloat(track.dataset.step) || 1;
        var unit = opts.unit;

        // Hydrate from URL or from the inputs (whichever has a value).
        var urlMin = readURL(opts.minInput.replace('#', ''));
        var urlMax = readURL(opts.maxInput.replace('#', ''));
        var startMin = urlMin !== null ? parseFloat(urlMin) : (minInput && minInput.value !== '' ? parseFloat(minInput.value) : min);
        var startMax = urlMax !== null ? parseFloat(urlMax) : (maxInput && maxInput.value !== '' ? parseFloat(maxInput.value) : max);
        if (isNaN(startMin)) startMin = min;
        if (isNaN(startMax)) startMax = max;

        var state = { lo: startMin, hi: startMax };

        function paint() {
            var range = max - min;
            var loPct = range === 0 ? 0 : ((state.lo - min) / range) * 100;
            var hiPct = range === 0 ? 100 : ((state.hi - min) / range) * 100;
            minThumb.style.left = loPct + '%';
            maxThumb.style.left = hiPct + '%';
            fill.style.left = loPct + '%';
            fill.style.width = (hiPct - loPct) + '%';
            minLabel.textContent = formatValue(state.lo, unit);
            maxLabel.textContent = formatValue(state.hi, unit);
            if (minInput) minInput.value = (state.lo === min) ? '' : roundForUnit(state.lo, unit);
            if (maxInput) maxInput.value = (state.hi === max) ? '' : roundForUnit(state.hi, unit);
        }

        function pointerToValue(clientX) {
            var rect = track.getBoundingClientRect();
            var pct = Math.max(0, Math.min(1, (clientX - rect.left) / rect.width));
            var raw = min + pct * (max - min);
            return snap(raw, step);
        }

        function drag(which, e) {
            e.preventDefault();
            var target = e.touches ? e.touches[0] : e;
            function move(ev) {
                var t = ev.touches ? ev.touches[0] : ev;
                var v = pointerToValue(t.clientX);
                if (which === 'lo') state.lo = Math.min(v, state.hi);
                else state.hi = Math.max(v, state.lo);
                paint();
                if (opts.onChange) opts.onChange();
            }
            function up() {
                document.removeEventListener('mousemove', move);
                document.removeEventListener('mouseup', up);
                document.removeEventListener('touchmove', move);
                document.removeEventListener('touchend', up);
            }
            document.addEventListener('mousemove', move);
            document.addEventListener('mouseup', up);
            document.addEventListener('touchmove', move, { passive: false });
            document.addEventListener('touchend', up);
        }

        minThumb.addEventListener('mousedown', function (e) { drag('lo', e); });
        maxThumb.addEventListener('mousedown', function (e) { drag('hi', e); });
        minThumb.addEventListener('touchstart', function (e) { drag('lo', e); }, { passive: false });
        maxThumb.addEventListener('touchstart', function (e) { drag('hi', e); }, { passive: false });

        // Click on track to move the nearest thumb.
        track.addEventListener('click', function (e) {
            if (e.target === minThumb || e.target === maxThumb) return;
            var v = pointerToValue(e.clientX);
            var dLo = Math.abs(v - state.lo);
            var dHi = Math.abs(v - state.hi);
            if (dLo <= dHi) { state.lo = Math.min(v, state.hi); }
            else { state.hi = Math.max(v, state.lo); }
            paint();
            if (opts.onChange) opts.onChange();
        });

        paint();
    }

    function snap(v, step) {
        if (!step || step <= 0) return v;
        return Math.round(v / step) * step;
    }

    function formatValue(v, unit) {
        if (unit === 'params') {
            if (v >= 1e9) return (v / 1e9).toFixed(v % 1e9 === 0 ? 0 : 1) + 'B';
            if (v >= 1e6) return (v / 1e6).toFixed(v % 1e6 === 0 ? 0 : 1) + 'M';
            if (v >= 1e3) return (v / 1e3).toFixed(v % 1e3 === 0 ? 0 : 1) + 'K';
            return String(v);
        }
        if (unit === 'date') {
            // stored as days-since-epoch
            var d = new Date(v * 86400000);
            return d.toLocaleDateString(undefined, { year: 'numeric', month: 'short' });
        }
        return String(v);
    }

    function roundForUnit(v, unit) {
        if (unit === 'params') return String(Math.round(v));
        if (unit === 'date') return daysToIso(v);
        return String(v);
    }

    function daysToIso(days) {
        var d = new Date(days * 86400000);
        return d.toISOString().slice(0, 10);
    }

    function isoToDays(iso) {
        var d = new Date(iso + 'T00:00:00Z');
        return Math.round(d.getTime() / 86400000);
    }

    // ------- Filtering ------------------------------------------------------
    function applyFilters() {
        var cards = document.querySelectorAll('.cards .card');
        if (!cards.length) return;

        var pMin = readRange('#params_min', '#params_max');
        var cMin = readRange('#created_after', '#created_before', isoToDays);
        var minDl = readNum('[data-filter-input="min_downloads"]');
        var minLikes = readNum('[data-filter-input="min_likes"]');

        var visible = 0;
        cards.forEach(function (card) {
            var ok = true;
            var params = parseInt(card.dataset.params || '0', 10);
            var dl = parseInt(card.dataset.downloads || '0', 10);
            var likes = parseInt(card.dataset.likes || '0', 10);
            var created = card.dataset.created || '';

            if (pMin !== null) {
                if (params === 0) ok = false; // unknown — hide when a range is set
                else if (params < pMin.min || params > pMin.max) ok = false;
            }
            if (ok && cMin !== null) {
                if (!created) ok = false;
                else {
                    var days = isoToDays(created);
                    if (days < cMin.min || days > cMin.max) ok = false;
                }
            }
            if (ok && minDl !== null) {
                if (dl < minDl) ok = false;
            }
            if (ok && minLikes !== null) {
                if (likes < minLikes) ok = false;
            }
            if (ok) {
                card.classList.remove('filtered-out');
                visible++;
            } else {
                card.classList.add('filtered-out');
            }
        });

        updateVisibleCount(visible, cards.length);
        updateRefineBadge(!!pMin, !!cMin, minDl !== null, minLikes !== null);
        updateRefineChips();
    }

    function readRange(minSel, maxSel, transform) {
        var minV = readControl(minSel, transform);
        var maxV = readControl(maxSel, transform);
        if (minV === null && maxV === null) return null;
        // Defaults: params slider is bounded 0..1e12; dates by URL/data attrs.
        var min = minV !== null ? minV : (transform ? 0 : 0);
        var max = maxV !== null ? maxV : (transform ? 100000 : 1e13);
        return { min: min, max: max };
    }

    function readControl(sel, transform) {
        var el = document.querySelector(sel);
        if (!el) return null;
        var v = el.value.trim();
        if (v === '') return null;
        var n = parseFloat(v);
        if (isNaN(n)) return null;
        return transform ? transform(v) : n;
    }

    function readNum(sel) {
        var el = document.querySelector(sel);
        if (!el) return null;
        var v = el.value.trim();
        if (v === '') return null;
        var n = parseInt(v, 10);
        if (isNaN(n) || n < 0) return null;
        return n;
    }

    function updateVisibleCount(visible, total) {
        var meta = document.getElementById('results-meta');
        if (!meta) return;
        if (visible === total) {
            meta.textContent = total + ' models';
            meta.classList.remove('refined');
        } else {
            meta.textContent = visible + ' of ' + total + ' shown';
            meta.classList.add('refined');
        }
    }

    // updateRefineBadge toggles the inline "X of N shown / Clear" notice above
    // the result grid. The badge is always present in the DOM (hidden by default)
    // so we just flip the hidden attribute and update the summary text.
    function updateRefineBadge(p, c, d, l) {
        var badge = document.getElementById('refine-badge');
        if (!badge) return;
        var text = document.getElementById('refine-badge-text');
        var active = p || c || d || l;
        if (!active) {
            badge.setAttribute('hidden', '');
            if (text) text.textContent = 'All results shown';
            return;
        }
        badge.removeAttribute('hidden');
        if (text) {
            var bits = [];
            if (p) bits.push('params');
            if (c) bits.push('date');
            if (d) bits.push('downloads');
            if (l) bits.push('likes');
            text.textContent = 'Refined by ' + bits.join(' · ');
        }
    }

    // updateRefineChips keeps the inline threshold chips next to the numeric
    // min_* inputs in sync with the typed value. Empty value -> hide the chip.
    function updateRefineChips() {
        document.querySelectorAll('[data-filter-input]').forEach(function (input) {
            var chip = document.querySelector('[data-filter-chip="' + input.name + '"]');
            if (!chip) return;
            var raw = input.value.trim();
            if (raw === '' || parseInt(raw, 10) <= 0) {
                chip.setAttribute('hidden', '');
                chip.textContent = '';
            } else {
                chip.removeAttribute('hidden');
                chip.textContent = '\u2265 ' + formatCompact(parseInt(raw, 10));
            }
        });
    }

    // formatCompact mirrors the Go-side helper so the chip shows the same
    // compact representation the user will see after a server-render.
    function formatCompact(n) {
        if (n >= 1e9)  return trimFloat(n / 1e9) + 'B';
        if (n >= 1e6)  return trimFloat(n / 1e6) + 'M';
        if (n >= 1e3)  return trimFloat(n / 1e3) + 'K';
        return String(n);
    }

    function trimFloat(f) {
        var s = f.toFixed(2);
        if (s.indexOf('.') >= 0) {
            s = s.replace(/0+$/, '').replace(/\.$/, '');
        }
        return s;
    }

    function clearAllControls() {
        document.querySelectorAll('[data-filter-input]').forEach(function (el) {
            el.value = '';
        });
        // Sliders keep state in their closures — easiest to reload from URL-less state.
        var p = document.querySelector('#params-range');
        var c = document.querySelector('#created-range');
        if (p && p.dataset) {
            var pmin = parseFloat(p.dataset.min);
            var pmax = parseFloat(p.dataset.max);
            var state = p._state || (p._state = {});
            state.lo = pmin;
            state.hi = pmax;
            repaintDual(p);
        }
        if (c && c.dataset) {
            var cmin = parseFloat(c.dataset.min);
            var cmax = parseFloat(c.dataset.max);
            var state2 = c._state || (c._state = {});
            state2.lo = cmin;
            state2.hi = cmax;
            repaintDual(c);
        }
    }

    function repaintDual(track) {
        var minThumb = track.querySelector('.range-thumb.range-min');
        var maxThumb = track.querySelector('.range-thumb.range-max');
        var fill = track.querySelector('.range-fill');
        var minLabel = track.querySelector('.range-label-min');
        var maxLabel = track.querySelector('.range-label-max');
        var unit = track.dataset.unit || 'params';
        var s = track._state;
        if (!s) return;
        var min = parseFloat(track.dataset.min);
        var max = parseFloat(track.dataset.max);
        var range = max - min;
        var loPct = range === 0 ? 0 : ((s.lo - min) / range) * 100;
        var hiPct = range === 0 ? 100 : ((s.hi - min) / range) * 100;
        minThumb.style.left = loPct + '%';
        maxThumb.style.left = hiPct + '%';
        fill.style.left = loPct + '%';
        fill.style.width = (hiPct - loPct) + '%';
        minLabel.textContent = formatValue(s.lo, unit);
        maxLabel.textContent = formatValue(s.hi, unit);
        var minInput = document.querySelector(track.dataset.inputMin);
        var maxInput = document.querySelector(track.dataset.inputMax);
        if (minInput) minInput.value = '';
        if (maxInput) maxInput.value = '';
    }

    function readURL(name) {
        var m = window.location.search.match(new RegExp('[?&]' + name + '=([^&]*)'));
        return m ? decodeURIComponent(m[1]) : null;
    }

    function debounce(fn, wait) {
        var t;
        return function () {
            var args = arguments;
            clearTimeout(t);
            t = setTimeout(function () { fn.apply(null, args); }, wait);
        };
    }
})();
