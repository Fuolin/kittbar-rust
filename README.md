# kittbar-rust
a bar in kitty hyprland

I use "special:kitbar" as a workspace

please add:"
#new workspace
workspace = special:kitbar, persistent:true

#keybend is $mainMod + space
bind = $mainMod, space, togglespecialworkspace, kitbar

#open
exec-once = ~/app/kitbar &
"
into your hyprland.conf

if you want use monitor only you can enter "kitbar --monitor"
