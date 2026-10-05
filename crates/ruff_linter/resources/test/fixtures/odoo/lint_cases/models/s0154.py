class Hooks(http.Controller):
    @route(
        "/hook/<uuid>",
        type="http",
        auth="receiver",
        receiver="automation.rule:_receiver_for_webhook",
        csrf=False,
    )
    def hook(self, uuid, **kw):
        return request.admission.subject
