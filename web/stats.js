(function () {
  'use strict';

  var SVG_NS = 'http://www.w3.org/2000/svg';
  var KRS = (window.KRS = window.KRS || {});

  var MONTHS_SHORT = [
    'Oca', 'Şub', 'Mar', 'Nis', 'May', 'Haz',
    'Tem', 'Ağu', 'Eyl', 'Eki', 'Kas', 'Ara'
  ];

  var histDateFmt = new Intl.DateTimeFormat('tr-TR', {
    day: 'numeric',
    month: 'short',
    year: 'numeric'
  });

  function formatHistoryDate(yyyyMmDd) {
    var parts = String(yyyyMmDd || '').split('-');
    if (parts.length !== 3) return String(yyyyMmDd || '');
    var y = parseInt(parts[0], 10);
    var m = parseInt(parts[1], 10);
    var d = parseInt(parts[2], 10);
    if (!Number.isFinite(y) || !Number.isFinite(m) || !Number.isFinite(d)) {
      return String(yyyyMmDd || '');
    }
    return histDateFmt.format(new Date(y, m - 1, d));
  }

  function formatDuration(sec) {
    var s = Math.max(0, Math.floor(Number(sec) || 0));
    if (s < 60) {
      return s + ' sn';
    }
    var h = Math.floor(s / 3600);
    var m = Math.floor((s % 3600) / 60);
    var rem = s % 60;
    if (h > 0) {
      return h + ' sa ' + m + ' dk';
    }
    return m + ' dk ' + rem + ' sn';
  }

  function formatShortDate(yyyyMmDd) {
    var parts = String(yyyyMmDd || '').split('-');
    if (parts.length !== 3) return String(yyyyMmDd || '');
    var m = parseInt(parts[1], 10);
    var d = parseInt(parts[2], 10);
    var mName = MONTHS_SHORT[(m - 1) % 12] || parts[1];
    return d + ' ' + mName;
  }

  function computeNiceTicks(maxVal) {
    if (maxVal <= 0) {
      return { top: 1, ticks: [0, 1], empty: true };
    }
    var bases = [1, 2, 5];
    var exp = 0;
    var step = 1;
    while (true) {
      var mult = Math.pow(10, exp);
      var found = false;
      for (var b = 0; b < bases.length; b++) {
        var cand = bases[b] * mult;
        if (Math.ceil(maxVal / cand) <= 4) {
          step = cand;
          found = true;
          break;
        }
      }
      if (found) break;
      exp += 1;
    }
    var count = Math.max(1, Math.ceil(maxVal / step));
    var ticks = [];
    for (var i = 0; i <= count; i++) {
      ticks.push(i * step);
    }
    return { top: count * step, ticks: ticks, empty: false };
  }

  // opts (ritim grafikleri): { labelFn(d), maxLabels } - etiket metni ve dar ekranda en çok etiket sayısı
  function buildBarChart(titleText, ariaLabel, daily, valueFn, unitLabel, cleanups, mount, opts) {
    var el = KRS.el;
    var clearNode = KRS.clearNode;
    var svg = document.createElementNS(SVG_NS, 'svg');
    svg.setAttribute('role', 'img');
    svg.setAttribute('aria-label', ariaLabel);

    var values = [];
    var maxVal = 0;
    for (var i = 0; i < daily.length; i++) {
      var v = Math.max(0, Number(valueFn(daily[i])) || 0);
      values.push(v);
      if (v > maxVal) maxVal = v;
    }

    var tickInfo = computeNiceTicks(maxVal);
    var topVal = tickInfo.top;
    var ticks = tickInfo.ticks;

    var card = el('section', { className: 'chart-card' }, [
      el('h2', { text: titleText }),
      svg
    ]);

    function renderSvg(rawW) {
      clearNode(svg);
      var w = Math.max(240, Math.floor(rawW || 600));
      var h = 200;
      svg.setAttribute('width', String(w));
      svg.setAttribute('height', String(h));
      svg.setAttribute('viewBox', '0 0 ' + w + ' ' + h);

      var leftPad = 36;
      var rightPad = 14;
      var topPad = 16;
      var bottomPad = 28;
      var plotW = Math.max(100, w - leftPad - rightPad);
      var plotH = h - topPad - bottomPad;

      for (var s = 0; s < ticks.length; s++) {
        var tVal = ticks[s];
        var ratio = topVal > 0 ? tVal / topVal : 0;
        var y = topPad + plotH * (1 - ratio);
        var line = document.createElementNS(SVG_NS, 'line');
        line.setAttribute('class', 'grid-line');
        line.setAttribute('x1', String(leftPad));
        line.setAttribute('x2', String(w - rightPad));
        line.setAttribute('y1', y.toFixed(2));
        line.setAttribute('y2', y.toFixed(2));
        svg.appendChild(line);

        var lbl = document.createElementNS(SVG_NS, 'text');
        lbl.setAttribute('class', 'axis-label');
        lbl.setAttribute('x', String(leftPad - 6));
        lbl.setAttribute('y', (y + 4).toFixed(2));
        lbl.setAttribute('text-anchor', 'end');
        lbl.textContent = String(tVal);
        svg.appendChild(lbl);
      }

      var n = Math.max(1, daily.length);
      var slotW = plotW / n;
      var barW = Math.max(3, slotW * 0.68);

      var labelSet = {};
      if (opts && opts.labelFn) {
        var stepL =
          opts.maxLabels && w < 600 && n > opts.maxLabels ? Math.ceil(n / opts.maxLabels) : 1;
        for (var kk = 0; kk < n; kk += stepL) labelSet[kk] = true;
      } else if (n > 0) {
        if (w < 600) {
          labelSet[0] = true;
          labelSet[Math.floor((n - 1) / 2)] = true;
          labelSet[n - 1] = true;
        } else {
          labelSet[0] = true;
          labelSet[Math.round((n - 1) * 0.25)] = true;
          labelSet[Math.round((n - 1) * 0.5)] = true;
          labelSet[Math.round((n - 1) * 0.75)] = true;
          labelSet[n - 1] = true;
        }
      }

      for (var j = 0; j < daily.length; j++) {
        var val = values[j];
        var bx = leftPad + j * slotW + (slotW - barW) / 2;
        var shortD = opts && opts.labelFn ? opts.labelFn(daily[j]) : formatShortDate(daily[j].date);

        if (val > 0) {
          var barH = Math.max(4, (val / topVal) * plotH);
          var by = topPad + plotH - barH;
          var rect = document.createElementNS(SVG_NS, 'rect');
          rect.setAttribute(
            'class',
            !opts && j === daily.length - 1 ? 'bar today' : 'bar'
          );
          rect.setAttribute('x', bx.toFixed(2));
          rect.setAttribute('y', by.toFixed(2));
          rect.setAttribute('width', barW.toFixed(2));
          rect.setAttribute('height', barH.toFixed(2));
          rect.setAttribute('rx', '3');

          var t = document.createElementNS(SVG_NS, 'title');
          t.textContent = shortD + ': ' + val + ' ' + unitLabel;
          rect.appendChild(t);
          svg.appendChild(rect);
        }

        if (labelSet[j]) {
          var xl = document.createElementNS(SVG_NS, 'text');
          xl.setAttribute('class', 'axis-label');
          var cx = bx + barW / 2;
          var clampedX = Math.max(leftPad + 16, Math.min(w - rightPad - 16, cx));
          xl.setAttribute('x', clampedX.toFixed(2));
          xl.setAttribute('y', String(h - 8));
          xl.setAttribute('text-anchor', 'middle');
          xl.textContent = shortD;
          svg.appendChild(xl);
        }
      }

      if (tickInfo.empty) {
        var emptyTxt = document.createElementNS(SVG_NS, 'text');
        emptyTxt.setAttribute('class', 'axis-label');
        emptyTxt.setAttribute('x', (leftPad + plotW / 2).toFixed(2));
        emptyTxt.setAttribute('y', (topPad + plotH / 2 + 4).toFixed(2));
        emptyTxt.setAttribute('text-anchor', 'middle');
        emptyTxt.textContent = 'Henüz veri yok';
        svg.appendChild(emptyTxt);
      }
    }

    var initW = ((mount && mount.clientWidth) || (typeof window !== 'undefined' && window.innerWidth) || 640) - 64;
    renderSvg(initW);

    var resizeTimer = null;
    var ro = null;
    if (typeof ResizeObserver !== 'undefined') {
      ro = new ResizeObserver(function () {
        if (resizeTimer) clearTimeout(resizeTimer);
        resizeTimer = setTimeout(function () {
          var cw = card.clientWidth ? card.clientWidth - 32 : 0;
          if (cw > 0) renderSvg(cw);
        }, 150);
      });
      ro.observe(card);
    }

    if (Array.isArray(cleanups)) {
      cleanups.push(function () {
        if (resizeTimer) clearTimeout(resizeTimer);
        if (ro) ro.disconnect();
      });
    }

    return card;
  }

  var MONTHS_LONG = [
    'Ocak', 'Şubat', 'Mart', 'Nisan', 'Mayıs', 'Haziran',
    'Temmuz', 'Ağustos', 'Eylül', 'Ekim', 'Kasım', 'Aralık'
  ];
  var WEEKDAYS = ['Pzt', 'Sal', 'Çar', 'Per', 'Cum', 'Cmt', 'Paz'];
  var CELL = 12;
  var GAP = 3;
  var LEFT = 28;
  var TOP = 20;

  function svgEl(name, attrs) {
    var n = document.createElementNS(SVG_NS, name);
    var keys = Object.keys(attrs || {});
    for (var i = 0; i < keys.length; i++) n.setAttribute(keys[i], String(attrs[keys[i]]));
    return n;
  }

  // Donmuş gün deseni (çizgili). id: aynı belgede tekil olmalı.
  function frozenDefs(id) {
    var defs = svgEl('defs', {});
    var p = svgEl('pattern', {
      id: id,
      width: 4,
      height: 4,
      patternUnits: 'userSpaceOnUse',
      patternTransform: 'rotate(45)'
    });
    p.appendChild(svgEl('rect', { class: 'hm-frozen-bg', width: 4, height: 4 }));
    p.appendChild(svgEl('rect', { class: 'hm-frozen-stripe', width: 2, height: 4 }));
    defs.appendChild(p);
    return defs;
  }

  function minutes(sec) {
    return sec > 0 ? Math.max(1, Math.round(sec / 60)) : 0;
  }

  // Yıl ısı haritası ve yılın ritmi; yıl değişince ikisi birlikte yenilenir.
  function buildYearViews(heatMount, rhythmMount, gen, hooks) {
    var el = KRS.el;
    var icon = KRS.icon;
    var clearNode = KRS.clearNode;
    var calc = window.KRS_CALC;
    var thisYear = parseInt(KRS.todayLocal().slice(0, 4), 10);
    var year = thisYear;
    var chartCleanups = [];
    var seq = 0;

    var yearSelect = el('select', { id: 'heatmap-year', className: 'hm-select' });
    var scroll = el('div', {
      className: 'heatmap-scroll',
      role: 'region',
      'aria-label': 'Okuma takvimi',
      tabindex: '0'
    });
    var body = el('div', { className: 'heatmap-body' });
    var legend = el('div', { className: 'hm-legend type-label-small' });
    var legendSvg = svgEl('svg', {
      width: 5 * (CELL + GAP) - GAP,
      height: CELL,
      viewBox: '0 0 ' + (5 * (CELL + GAP) - GAP) + ' ' + CELL
    });
    for (var li = 0; li < 5; li++) {
      legendSvg.appendChild(
        svgEl('rect', {
          class: 'hm-cell hm-' + li,
          x: li * (CELL + GAP),
          y: 0,
          width: CELL,
          height: CELL,
          rx: 2
        })
      );
    }
    legend.appendChild(el('span', { text: 'Az' }));
    legend.appendChild(legendSvg);
    legend.appendChild(el('span', { text: 'Çok' }));
    // Durum öğeleri: yalnız renkle değil, kenarlık/desen/soluklukla da ayrışır
    function legendItem(label, shape) {
      var sw = svgEl('svg', { width: CELL, height: CELL, viewBox: '0 0 ' + CELL + ' ' + CELL });
      if (shape === 'frozen') sw.appendChild(frozenDefs('hm-frozen-pat-lg'));
      var r = svgEl('rect', {
        class: shape === 'qualified' ? 'hm-cell hm-3 hm-cur' : shape === 'frozen' ? 'hm-cell hm-frozen' : 'hm-cell hm-1 hm-weak',
        x: 0,
        y: 0,
        width: CELL,
        height: CELL,
        rx: 2
      });
      if (shape === 'frozen') r.setAttribute('fill', 'url(#hm-frozen-pat-lg)');
      sw.appendChild(r);
      return el('span', { className: 'lg' }, [sw, el('span', { text: label })]);
    }
    legend.appendChild(legendItem('Seriye sayıldı', 'qualified'));
    legend.appendChild(legendItem('Donmuş gün', 'frozen'));
    legend.appendChild(legendItem('Eşik altı', 'weak'));

    heatMount.appendChild(
      el('div', { className: 'heatmap-head' }, [
        el('h2', { className: 'type-title-medium', text: 'Okuma takvimi' }),
        el('div', { className: 'hm-year' }, [
          el('label', { for: 'heatmap-year', className: 'type-label-large', text: 'Yıl' }),
          yearSelect
        ])
      ])
    );
    heatMount.appendChild(body);

    var rhythmTitle = el('h2', { className: 'type-title-medium', text: 'Yılın ritmi' });
    rhythmMount.appendChild(rhythmTitle);
    var rhythmBody = el('div', { className: 'rhythm-body' });
    rhythmMount.appendChild(rhythmBody);

    function fillYears(years) {
      var set = {};
      (years || []).forEach(function (y) {
        set[y] = true;
      });
      set[thisYear] = true;
      set[year] = true;
      var list = Object.keys(set)
        .map(Number)
        .sort(function (a, b) {
          return b - a;
        });
      clearNode(yearSelect);
      list.forEach(function (y) {
        var o = el('option', { value: String(y), text: String(y) });
        if (y === year) o.selected = true;
        yearSelect.appendChild(o);
      });
    }

    function runChartCleanups() {
      chartCleanups.forEach(function (c) {
        c();
      });
      chartCleanups = [];
    }

    // Hücre boyutu kap genişliğine göre: clamp(12, floor((w - 28 - (cols-1)*gap)/cols), 20)
    function cellSize(cols) {
      var w = body.clientWidth || scroll.clientWidth || 0;
      if (!w) return CELL;
      var c = Math.floor((w - LEFT - (cols - 1) * GAP) / cols);
      return Math.max(CELL, Math.min(20, c));
    }

    var lastData = null;
    var lastCell = 0;

    function drawHeatmap(data, first) {
      lastData = data;
      var grid = calc.yearGrid(year);
      var cs = cellSize(grid.cols);
      lastCell = cs;
      var step = cs + GAP;
      var prevScroll = scroll.scrollLeft;
      var byDate = {};
      var values = [];
      (data.days || []).forEach(function (d) {
        byDate[d.date] = d;
        values.push(Number(d.seconds) || 0);
      });
      var th = calc.heatThresholds(values);
      var width = LEFT + grid.cols * cs + (grid.cols - 1) * GAP;
      var height = TOP + 7 * cs + 6 * GAP;
      var svg = svgEl('svg', {
        width: width,
        height: height,
        viewBox: '0 0 ' + width + ' ' + height
      });
      svg.appendChild(frozenDefs('hm-frozen-pat'));
      grid.months.forEach(function (m) {
        var t = svgEl('text', { class: 'axis-label', x: LEFT + m.col * step, y: 12 });
        t.textContent = m.label;
        svg.appendChild(t);
      });
      [0, 2, 4].forEach(function (r) {
        var t = svgEl('text', {
          class: 'axis-label',
          x: 0,
          y: TOP + r * step + cs - 2
        });
        t.textContent = WEEKDAYS[r];
        svg.appendChild(t);
      });

      var cells = [];
      var today = KRS.todayLocal();
      var focusIdx = 0;
      grid.cells.forEach(function (c, idx) {
        var d = byDate[c.date];
        var sec = d ? Number(d.seconds) || 0 : 0;
        var level = calc.heatLevel(sec, th);
        var isQ = !!(d && d.qualified), isFz = !!(d && d.frozen), inCur = !!(d && d.in_current);
        var isWeak = !!(d && !d.qualified && !d.frozen && sec > 0);
        var suffix = (isQ ? ', seriye sayıldı' : isFz ? ', seri dondurma günü' : isWeak ? ', eşiğin altında, seriye sayılmadı' : '') +
          (inCur ? ', güncel seride' : '');
        var label = formatHistoryDate(c.date) + ': ' +
          (sec > 0 ? formatDuration(sec) + ', ' + (d.pages || 0) + ' sayfa' : 'okuma yok') + suffix;
        var cellAttrs = {
          // donmuş gün: dolgu desen (CSS hm-N dolgusu deseni ezmesin diye seviye sınıfı yok)
          class: 'hm-cell' + (isFz ? ' hm-frozen' : ' hm-' + level) + (isQ || isFz ? ' hm-link' : '') +
            (inCur ? ' hm-cur' : '') + (isWeak ? ' hm-weak' : ''),
          x: LEFT + c.col * step,
          y: TOP + c.row * step,
          width: cs,
          height: cs,
          rx: 2,
          role: 'button',
          tabindex: '-1',
          'aria-label': label,
          'data-date': c.date
        };
        if (isFz) cellAttrs.fill = 'url(#hm-frozen-pat)';
        var rect = svgEl('rect', cellAttrs);
        var title = svgEl('title', {});
        title.textContent = label;
        rect.appendChild(title);
        if (hooks.getDate() === c.date) {
          rect.classList.add('selected');
          focusIdx = idx;
        } else if (!hooks.getDate() && c.date === today) {
          focusIdx = idx;
        }
        rect.addEventListener('click', function () {
          pick(idx);
        });
        cells.push(rect);
        svg.appendChild(rect);
      });

      // Zincir bağlantıları: hücrelerin ALTINA çizilir (önce eklenir)
      var linkG = svgEl('g', {});
      calc.chainLinks(grid.cells, byDate).forEach(function (l) {
        var ca = grid.cells[l.a], cb = grid.cells[l.b];
        var barCls = 'hm-link-bar' + (l.current ? ' hm-link-cur' : '');
        if (!l.wrap) {
          linkG.appendChild(svgEl('rect', { class: barCls, x: LEFT + ca.col * step, y: TOP + ca.row * step + cs, width: cs, height: GAP }));
        } else {
          // Pazar -> sonraki Pazartesi: Pazar altında ve sonraki sütunun üstünde çıkıntı
          linkG.appendChild(svgEl('rect', { class: barCls, x: LEFT + ca.col * step, y: TOP + 6 * step + cs, width: cs, height: GAP }));
          linkG.appendChild(svgEl('rect', { class: barCls, x: LEFT + cb.col * step, y: TOP - GAP, width: cs, height: GAP }));
        }
      });
      svg.insertBefore(linkG, svg.firstChild);

      var cur = focusIdx;
      cells[cur].setAttribute('tabindex', '0');
      function moveTo(i) {
        var n = Math.max(0, Math.min(cells.length - 1, i));
        cells[cur].setAttribute('tabindex', '-1');
        cur = n;
        cells[cur].setAttribute('tabindex', '0');
        cells[cur].focus();
      }
      function pick(i) {
        var date = grid.cells[i].date;
        hooks.setDate(hooks.getDate() === date ? null : date);
        cells.forEach(function (c, k) {
          c.classList.toggle('selected', hooks.getDate() === grid.cells[k].date);
        });
        moveTo(i);
      }
      svg.addEventListener('keydown', function (e) {
        var step = null;
        if (e.key === 'ArrowLeft') step = -7;
        else if (e.key === 'ArrowRight') step = 7;
        else if (e.key === 'ArrowUp') step = -1;
        else if (e.key === 'ArrowDown') step = 1;
        if (step !== null) {
          e.preventDefault();
          moveTo(cur + step);
        } else if (e.key === 'Home') {
          e.preventDefault();
          moveTo(0);
        } else if (e.key === 'End') {
          e.preventDefault();
          moveTo(cells.length - 1);
        } else if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault();
          pick(cur);
        }
      });

      clearNode(scroll);
      scroll.appendChild(svg);
      clearNode(body);
      body.appendChild(scroll);
      body.appendChild(legend);
      if (!first) {
        scroll.scrollLeft = prevScroll;
      } else if (year === thisYear) {
        // içinde bulunulan yıl: bugünün sütunu görünür ortalanır
        var todayCell = grid.cells.filter(function (c) {
          return c.date === today;
        })[0];
        if (todayCell) {
          var cx = LEFT + todayCell.col * step + cs / 2;
          scroll.scrollLeft = Math.max(0, cx - scroll.clientWidth / 2);
        } else {
          scroll.scrollLeft = scroll.scrollWidth;
        }
      } else {
        // geçmiş yıl: sona kaydırılır
        scroll.scrollLeft = scroll.scrollWidth;
      }
    }

    function drawRhythm(data) {
      runChartCleanups();
      clearNode(rhythmBody);
      var days = data.days || [];
      var sum = calc.yearSummary(days);
      if (sum.daysRead === 0) {
        rhythmBody.appendChild(el('p', { className: 'type-body-medium rhythm-empty', text: 'Bu yıl için veri yok' }));
        return;
      }
      var mt = calc.monthTotals(days);
      var wt = calc.weekdayTotals(days);
      var monthData = mt.map(function (sec, i) {
        return { label: MONTHS_SHORT[i], seconds: sec };
      });
      var weekData = wt.map(function (sec, i) {
        return { label: WEEKDAYS[i], seconds: sec };
      });
      function valueFn(d) {
        return minutes(d.seconds);
      }
      function labelFn(d) {
        return d.label;
      }
      rhythmBody.appendChild(
        buildBarChart('Aylara göre okuma (dk)', year + ' yılında aya göre okuma süresi', monthData, valueFn, 'dk', chartCleanups, rhythmBody, { labelFn: labelFn, maxLabels: 6 })
      );
      rhythmBody.appendChild(
        buildBarChart('Haftanın günlerine göre okuma (dk)', year + ' yılında haftanın gününe göre okuma süresi', weekData, valueFn, 'dk', chartCleanups, rhythmBody, { labelFn: labelFn })
      );
      function item(term, value, help) {
        return el('div', { className: 'summary-item' }, [
          el('dt', { className: 'type-label-medium', text: term }),
          el('dd', {}, [
            el('span', { className: 'type-title-medium summary-value', text: value }),
            help ? el('span', { className: 'type-body-small summary-help', text: help }) : null
          ])
        ]);
      }
      rhythmBody.appendChild(
        el('dl', { className: 'summary-grid' }, [
          item('En çok okuduğun ay', MONTHS_LONG[sum.bestMonth] + ' · ' + formatDuration(sum.bestMonthSeconds)),
          item('En çok okuduğun gün', formatHistoryDate(sum.bestDay) + ' · ' + formatDuration(sum.bestDaySeconds)),
          item('Okuduğun gün sayısı', sum.daysRead + ' gün'),
          item('Günlük ortalama', formatDuration(Math.round(sum.avgPerDay)), 'Okuduğun günler üzerinden hesaplanır.')
        ])
      );
    }

    async function load() {
      var mine = ++seq;
      clearNode(body);
      for (var k = 0; k < 2; k++) body.appendChild(el('div', { className: 'skeleton skeleton-row' }));
      try {
        var data = await KRS.api('GET', '/api/stats/heatmap?year=' + year + '&today=' + encodeURIComponent(KRS.todayLocal()));
        if (mine !== seq || (gen !== undefined && gen !== KRS.routeGen)) return;
        fillYears(data.years);
        drawHeatmap(data, true);
        drawRhythm(data);
      } catch (err) {
        if (mine !== seq || (gen !== undefined && gen !== KRS.routeGen)) return;
        clearNode(body);
        body.appendChild(
          el('div', { className: 'empty error' }, [
            icon('error'),
            el('h2', { text: 'Yüklenemedi' }),
            el('p', { text: err.message }),
            el('button', {
              type: 'button',
              className: 'btn tonal',
              text: 'Yeniden dene',
              onClick: load
            })
          ])
        );
      }
    }

    yearSelect.addEventListener('change', function () {
      year = parseInt(yearSelect.value, 10);
      load();
    });
    fillYears([]);
    load();

    // genişlik değişince hücre boyutu yeniden hesaplanır (150 ms debounce)
    var roTimer = null;
    var ro = null;
    if (typeof ResizeObserver !== 'undefined') {
      ro = new ResizeObserver(function () {
        if (roTimer) clearTimeout(roTimer);
        roTimer = setTimeout(function () {
          if (!lastData) return;
          var cs = cellSize(calc.yearGrid(year).cols);
          if (cs !== lastCell) drawHeatmap(lastData, false);
        }, 150);
      });
      ro.observe(body);
    }

    return function cleanup() {
      seq++;
      if (roTimer) clearTimeout(roTimer);
      if (ro) ro.disconnect();
      runChartCleanups();
    };
  }

  async function renderHistory(mount, gen) {
    var myGen = gen !== undefined ? gen : KRS.routeGen;
    var el = KRS.el;
    var icon = KRS.icon;
    var clearNode = KRS.clearNode;
    clearNode(mount);

    var limit = 50;
    var offset = 0;
    var total = 0;
    var dateFilter = null;
    var listSeq = 0;

    var heatCard = el('section', { className: 'section-card heatmap-card' });
    var rhythm = el('section', { className: 'rhythm' });
    var chipWrap = el('div', { className: 'history-filter' });
    var listArea = el('div', { className: 'history-list' });
    mount.appendChild(heatCard);
    mount.appendChild(rhythm);
    mount.appendChild(chipWrap);
    mount.appendChild(listArea);

    var cleanupYear = buildYearViews(heatCard, rhythm, myGen, {
      getDate: function () {
        return dateFilter;
      },
      setDate: function (d) {
        dateFilter = d;
        renderChip();
        reload();
      }
    });

    function renderChip() {
      clearNode(chipWrap);
      if (!dateFilter) return;
      chipWrap.appendChild(
        el(
          'button',
          {
            type: 'button',
            className: 'chip',
            'aria-label': 'Gün süzgecini kaldır',
            onClick: function () {
              dateFilter = null;
              renderChip();
              document.querySelectorAll('.hm-cell.selected').forEach(function (c) {
                c.classList.remove('selected');
              });
              reload();
            }
          },
          [formatHistoryDate(dateFilter) + ' ×']
        )
      );
    }

    var ul = el('ul', { id: 'history-table', className: 'list' });
    var moreWrap = el('div', { className: 'setting-row' });
    var moreBtn = null;

    function reload() {
      offset = 0;
      total = 0;
      ul = el('ul', { id: 'history-table', className: 'list' });
      moreBtn = null;
      moreWrap = el('div', { className: 'setting-row' });
      loadPage(true);
    }

    async function loadPage(firstLoad) {
      var mine = ++listSeq;
      if (firstLoad) {
        clearNode(listArea);
        var skelWrap = el('div', { className: 'list' });
        for (var s = 0; s < 4; s++) {
          skelWrap.appendChild(el('div', { className: 'skeleton skeleton-row' }));
        }
        listArea.appendChild(skelWrap);
      } else if (moreBtn) {
        moreBtn.disabled = true;
      }
      try {
        var res = await KRS.api(
          'GET',
          '/api/history?limit=' + limit + '&offset=' + offset +
            (dateFilter ? '&date=' + encodeURIComponent(dateFilter) : '')
        );
        if ((myGen !== undefined && myGen !== KRS.routeGen) || mine !== listSeq) {
          return;
        }
        if (firstLoad) {
          clearNode(listArea);
        }
        var items = (res && res.items) || [];
        total = Number((res && res.total) || 0);

        if (firstLoad && items.length === 0) {
          listArea.appendChild(ul);
          listArea.appendChild(
            el('div', { className: 'empty' }, [
              icon('history'),
              el('h2', {
                text: dateFilter ? 'Bu günde okuma kaydı yok' : 'Henüz okuma kaydı yok'
              })
            ])
          );
          return;
        }

        if (firstLoad) {
          listArea.appendChild(ul);
          listArea.appendChild(moreWrap);
        }

        for (var i = 0; i < items.length; i++) {
          var it = items[i];
          var titleChildren = [];
          if (it.book_missing) {
            titleChildren.push(
              el('span', { className: 'badge', text: '(silinmiş)' }),
              ' '
            );
          }
          titleChildren.push(it.book_title);

          var startPct = Math.round((it.start_percent || 0) * 100);
          var endPct = Math.round((it.end_percent || 0) * 100);
          var pctText =
            startPct === endPct
              ? '%' + endPct
              : '%' + startPct + ' → %' + endPct;
          var pctSpan = el('span', { className: 'nowrap', text: pctText });
          pctSpan.style.whiteSpace = 'nowrap';

          var subPrefix =
            it.series_name +
            ' · ' +
            formatHistoryDate(it.local_date) +
            ' · ' +
            formatDuration(it.active_seconds) +
            ' · ' +
            it.pages_read +
            ' sayfa · ';

          var li = el('li', { className: 'list-item' }, [
            el('div', { className: 'list-content' }, [
              el('h2', { className: 'list-title' }, titleChildren),
              el('p', { className: 'list-sub' }, [subPrefix, pctSpan])
            ])
          ]);
          ul.appendChild(li);
        }

        offset += items.length;
        clearNode(moreWrap);
        moreBtn = null;
        if (offset < total) {
          moreBtn = el('button', {
            type: 'button',
            className: 'btn tonal',
            text: 'Daha fazla',
            onClick: function () {
              loadPage(false);
            }
          });
          moreWrap.appendChild(moreBtn);
        }
      } catch (err) {
        if ((myGen !== undefined && myGen !== KRS.routeGen) || mine !== listSeq) {
          return;
        }
        if (!firstLoad) {
          if (moreBtn) moreBtn.disabled = false;
          if (KRS.toast) KRS.toast(err.message);
          return;
        }
        clearNode(listArea);
        listArea.appendChild(
          el('div', { className: 'empty error' }, [
            icon('error'),
            el('h2', { text: 'Yüklenemedi' }),
            el('p', { text: err.message }),
            el('button', {
              type: 'button',
              className: 'btn tonal',
              text: 'Yeniden dene',
              onClick: function () {
                loadPage(true);
              }
            })
          ])
        );
      }
    }

    await loadPage(true);
    return cleanupYear;
  }

  async function renderStats(mount, gen) {
    var myGen = gen !== undefined ? gen : KRS.routeGen;
    var el = KRS.el;
    var icon = KRS.icon;
    var clearNode = KRS.clearNode;
    clearNode(mount);

    var skelWrap = el('div', { className: 'stat-grid' });
    for (var s = 0; s < 5; s++) {
      skelWrap.appendChild(el('div', { className: 'skeleton skeleton-row' }));
    }
    mount.appendChild(skelWrap);

    var today = KRS.todayLocal();
    try {
      var data = await KRS.api(
        'GET',
        '/api/stats?today=' + encodeURIComponent(today)
      );
      if (myGen !== undefined && myGen !== KRS.routeGen) {
        return;
      }
      clearNode(mount);

      var statItems = [
        {
          icon: 'schedule',
          value: formatDuration(data.total_seconds),
          label: 'Toplam süre'
        },
        {
          icon: 'auto_stories',
          value: String(data.total_pages || 0),
          label: 'Toplam sayfa'
        },
        {
          icon: 'check_circle',
          value: String(data.finished_count || 0),
          label: 'Tamamlanan kitap'
        }
      ];

      // Seri kartı: güncel seri, en uzun seri, bugün uyarısı ve dondurma hakkı
      var sm = window.KRS_CALC.streakMessage(data);
      mount.appendChild(
        el('section', { className: 'streak-card' + (sm.kind === 'risk' ? ' risk' : '') }, [
          el('div', { className: 'streak-head' }, [
            icon('local_fire_department'),
            el('p', { className: 'streak-num', text: (data.streak_current || 0) + ' gün' })
          ]),
          el('p', { className: 'streak-sub', text: 'En uzun seri: ' + (data.streak_longest || 0) + ' gün' }),
          el('p', { className: 'streak-msg', role: 'status' },
            sm.kind === 'risk' ? [icon('warning'), el('span', { text: sm.text })] : [el('span', { text: sm.text })]),
          el('p', { className: 'streak-freeze', text: sm.freezeText })
        ])
      );

      var statGrid = el('div', { className: 'stat-grid' });
      for (var i = 0; i < statItems.length; i++) {
        var st = statItems[i];
        statGrid.appendChild(
          el('div', { className: 'stat-card' }, [
            icon(st.icon),
            el('p', { className: 'stat-value', text: st.value }),
            el('p', { className: 'stat-label', text: st.label })
          ])
        );
      }
      mount.appendChild(statGrid);

      var daily = data.daily || [];
      var cleanups = [];
      var maxSec = 0;
      for (var dIdx = 0; dIdx < daily.length; dIdx++) {
        var ds = Number(daily[dIdx].seconds) || 0;
        if (ds > maxSec) maxSec = ds;
      }
      var useSeconds = maxSec > 0 && maxSec < 60;
      mount.appendChild(
        buildBarChart(
          useSeconds ? 'Günlük okuma süresi (sn)' : 'Günlük okuma süresi (dk)',
          'Son 30 günde günlük okuma süresi',
          daily,
          function (d) {
            var sec = Number(d.seconds) || 0;
            if (sec <= 0) return 0;
            return useSeconds ? Math.round(sec) : Math.max(1, Math.round(sec / 60));
          },
          useSeconds ? 'sn' : 'dk',
          cleanups,
          mount
        )
      );
      mount.appendChild(
        buildBarChart(
          'Günlük okunan sayfa',
          'Son 30 günde günlük okunan sayfa',
          daily,
          function (d) {
            return d.pages || 0;
          },
          'sayfa',
          cleanups,
          mount
        )
      );

      var goal = data.goal || {
        year: parseInt(today.slice(0, 4), 10),
        target: null,
        done: 0
      };
      var ratio =
        goal.target && goal.target > 0
          ? Math.min(1, (goal.done || 0) / goal.target)
          : 0;
      var ringSvg = KRS.createCircularProgress(ratio, true);
      var ringText = el('span', {
        className: 'goal-ring-text',
        text: (goal.done || 0) + '/' + (goal.target ? goal.target : '0')
      });
      var ringWrap = el('div', { className: 'goal-ring-wrap' }, [
        ringSvg,
        ringText
      ]);

      var goalField = KRS.createField({
        id: 'goal-target-input',
        label: 'Yıllık kitap hedefi',
        type: 'number',
        min: 1,
        max: 1000,
        value: goal.target ? String(goal.target) : '',
        required: true
      });

      var goalForm = el(
        'form',
        {
          className: 'setting-row',
          onSubmit: async function (e) {
            e.preventDefault();
            var tVal = parseInt(goalField.input.value, 10);
            if (!Number.isFinite(tVal) || tVal < 1 || tVal > 1000) {
              goalField.setError('1 ile 1000 arasında bir sayı girin.');
              return;
            }
            try {
              await KRS.api('PUT', '/api/goal', {
                year: goal.year,
                target: tVal
              });
              KRS.toast('Hedef kaydedildi');
              for (var c = 0; c < cleanups.length; c++) {
                cleanups[c]();
              }
              var nextCleanup = await renderStats(mount, myGen);
              if (myGen === KRS.routeGen && typeof nextCleanup === 'function') {
                KRS.setCleanup(nextCleanup);
              }
            } catch (err) {
              KRS.toast(err.message);
            }
          }
        },
        [
          goalField.wrap,
          el('button', {
            type: 'submit',
            className: 'btn filled',
            text: 'Hedefi kaydet'
          })
        ]
      );

      var goalStatusText = goal.target
        ? goal.year + ' yılı hedefi: ' + goal.done + ' / ' + goal.target + ' kitap'
        : 'Bu yıl için hedef belirleyin';

      var goalCard = el('section', { className: 'section-card' }, [
        el('div', { className: 'goal-card' }, [
          ringWrap,
          el('div', { className: 'section-stack' }, [
            el('div', { className: 'chip-group' }, [
              icon('flag'),
              el('h2', {
                className: 'type-title-medium',
                text: 'Yıllık okuma hedefi'
              })
            ]),
            el('p', { className: 'type-body-medium', text: goalStatusText }),
            goalForm
          ])
        ])
      ]);
      mount.appendChild(goalCard);
      return function () {
        for (var c = 0; c < cleanups.length; c++) {
          cleanups[c]();
        }
      };
    } catch (err) {
      if (myGen !== undefined && myGen !== KRS.routeGen) {
        return;
      }
      clearNode(mount);
      mount.appendChild(
        el('div', { className: 'empty error' }, [
          icon('error'),
          el('h2', { text: 'Yüklenemedi' }),
          el('p', { text: err.message }),
          el('button', {
            type: 'button',
            className: 'btn tonal',
            text: 'Yeniden dene',
            onClick: function () {
              renderStats(mount, myGen);
            }
          })
        ])
      );
    }
  }

  KRS.stats = {
    renderHistory: renderHistory,
    renderStats: renderStats
  };
})();
