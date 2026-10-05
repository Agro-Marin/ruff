
Markup(env._("<b>%(x)s</b>", x=name))
Markup(self.env._("<b>%s</b>", name))
Markup(_("<b>%s</b>") % name)
Markup("<b>%s</b>" % (name,))
markupsafe.Markup("<b>{}</b>".format(name))
Markup(f"<b>{name}</b>")
Markup("<br/>" + response["message"])
