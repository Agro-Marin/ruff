
Markup(env._("<b>%(x)s</b>")) % {"x": name}
Markup("<b>{}</b>").format(name)
Markup(_("<b>%s</b>", escape(name)))
Markup("<b>%s</b>" % "constant")
Markup(f"<br/>{'a' if flag else 'b'}")
Markup(f"<br/>")
Markup("<br/>" + escape(message) + "</p>")
Markup(body)
