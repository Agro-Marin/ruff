from markupsafe import Markup, escape

from odoo import _, models


class Order(models.Model):
    _name = "fixture.order"

    def bad(self, name, url):
        Markup("<b>" + name + "</b>")  # E8538
        Markup(self.env._("Hello %(name)s", name=name))  # E8538
        Markup(_("Hello %s", name))  # E8538
        Markup("<b>%s</b>" % name)  # E8538
        Markup(_("<b>%s</b>") % (name, url))  # E8538
        Markup("<a href='{}'>{}</a>".format(url, name))  # E8538
        Markup(f"<b>{name}</b>")  # E8538
        markupsafe.Markup("<b>%(n)s</b>" % {"n": name})  # E8538

    def good(self, name, flag):
        Markup("<b>%s</b>") % name  # OK: the template is wrapped
        Markup(self.env._("Hello %(name)s")) % {"name": name}  # OK
        Markup("<b>" + escape(name) + "</b>")  # OK: escaped
        Markup("<b>%s</b>" % ("x" if flag else Markup("<i>y</i>")))  # OK
        Markup(f"<b>constant</b>")  # OK: nothing interpolated
        Markup(_("Hello"))  # OK: a gettext call without values
        Markup("<b>%s</b>" % {**values})  # E8538: an unpacked mapping is one value
