-- Border colors follow the theme's palette; the rest of the look is yours.
local active = {{ hypr_gradient active_border accent }}
local inactive = {{ hypr_gradient inactive_border rgba(595959aa) }}

hl.config({
  general = {
    col = {
      active_border = active,
      inactive_border = inactive,
    },
  },
  group = {
    col = {
      border_active = active,
      border_inactive = inactive,
    },
  },
})
