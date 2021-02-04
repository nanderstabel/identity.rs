(function() {var implementors = {};
implementors["identity_core"] = [{"text":"impl DerefMut for Url","synthetic":false,"types":[]},{"text":"impl DerefMut for Signature","synthetic":false,"types":[]},{"text":"impl DerefMut for SignatureValue","synthetic":false,"types":[]}];
implementors["identity_credential"] = [{"text":"impl&lt;T&gt; DerefMut for VerifiableCredential&lt;T&gt;","synthetic":false,"types":[]},{"text":"impl&lt;T, U&gt; DerefMut for VerifiablePresentation&lt;T, U&gt;","synthetic":false,"types":[]}];
implementors["identity_did"] = [{"text":"impl&lt;T&gt; DerefMut for DIDKey&lt;T&gt;","synthetic":false,"types":[]},{"text":"impl&lt;T&gt; DerefMut for Properties&lt;T&gt;","synthetic":false,"types":[]}];
implementors["identity_iota"] = [{"text":"impl&lt;T&gt; DerefMut for MessageIndex&lt;T&gt;","synthetic":false,"types":[]}];
if (window.register_implementors) {window.register_implementors(implementors);} else {window.pending_implementors = implementors;}})()