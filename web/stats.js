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

  function buildBarChart(titleText, ariaLabel, daily, valueFn, unitLabel, cleanups, mount) {
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
      if (n > 0) {
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
        var shortD = formatShortDate(daily[j].date);

        if (val > 0) {
          var barH = Math.max(4, (val / topVal) * plotH);
          var by = topPad + plotH - barH;
          var rect = document.createElementNS(SVG_NS, 'rect');
          rect.setAttribute('class', j === daily.length - 1 ? 'bar today' : 'bar');
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

  async function renderHistory(mount, gen) {
    var myGen = gen !== undefined ? gen : KRS.routeGen;
    var el = KRS.el;
    var icon = KRS.icon;
    var clearNode = KRS.clearNode;
    clearNode(mount);

    var skelWrap = el('div', { className: 'list' });
    for (var s = 0; s < 4; s++) {
      skelWrap.appendChild(el('div', { className: 'skeleton skeleton-row' }));
    }
    mount.appendChild(skelWrap);

    var limit = 50;
    var offset = 0;
    var total = 0;

    var ul = el('ul', { id: 'history-table', className: 'list' });
    var moreWrap = el('div', { className: 'setting-row' });
    var moreBtn = null;

    async function loadPage(firstLoad) {
      if (!firstLoad && moreBtn) {
        moreBtn.disabled = true;
      }
      try {
        var res = await KRS.api(
          'GET',
          '/api/history?limit=' + limit + '&offset=' + offset
        );
        if (myGen !== undefined && myGen !== KRS.routeGen) {
          return;
        }
        if (firstLoad) {
          clearNode(mount);
        }
        var items = (res && res.items) || [];
        total = Number((res && res.total) || 0);

        if (firstLoad && items.length === 0) {
          mount.appendChild(ul);
          mount.appendChild(
            el('div', { className: 'empty' }, [
              icon('history'),
              el('h2', { text: 'Henüz okuma kaydı yok' })
            ])
          );
          return;
        }

        if (firstLoad) {
          mount.appendChild(ul);
          mount.appendChild(moreWrap);
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
        if (myGen !== undefined && myGen !== KRS.routeGen) {
          return;
        }
        if (!firstLoad) {
          if (moreBtn) moreBtn.disabled = false;
          if (KRS.toast) KRS.toast(err.message);
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
                renderHistory(mount, myGen);
              }
            })
          ])
        );
      }
    }

    await loadPage(true);
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
        },
        {
          icon: 'local_fire_department',
          value: (data.streak_current || 0) + ' gün',
          label: 'Güncel seri'
        },
        {
          icon: 'local_fire_department',
          value: (data.streak_longest || 0) + ' gün',
          label: 'En uzun seri'
        }
      ];

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
